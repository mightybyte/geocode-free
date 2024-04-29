use serde::{Deserialize, Serialize};
use std::{fs, io, thread, time};
use structopt::StructOpt;

#[derive(StructOpt)]
pub struct Opts {
    /// Name of file holding the geocode.maps.co api key
    #[structopt(short, long)]
    api_key_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GeoData {
    place_id: u64,
    licence: String,
    osm_type: Option<String>,
    osm_id: Option<u64>,
    //boundingbox: Vec<String>,
    lat: String,
    lon: String,
    display_name: String,
    class: String,
    r#type: String,
    importance: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AddrGeoData {
    /// The original address used for lookup
    address: String,
    place_id: u64,
    licence: String,
    osm_type: Option<String>,
    osm_id: Option<u64>,
    lat: String,
    lon: String,
    display_name: String,
    class: String,
    r#type: String,
    importance: f64,
}

// https://geocode.maps.co/search?q=address&api_key=api_key
// Free for 1M requests / month @ 1 request / second

async fn get_latlons(api_key: &str, addr: &String) -> Result<Vec<GeoData>, String> {
    let res = reqwest::get(format!("https://geocode.maps.co/search?q={}&api_key={}", addr, api_key)).await;
    match res {
        Err(e) => Err(format!("Error in lat/lon query: {:?}", e)),
        Ok(resp) => {
            //let s = resp.text().await.unwrap();
            match resp.text().await {
                Err(e) => Err(format!("Error in query: {}", e)),
                Ok(s) => {
                    if s.len() == 0 {
                        return Err(format!("Empty response for address {}", addr));
                    } else {
                        let res: Result<Vec<GeoData>, String> = serde_json::from_str(s.as_str()).map_err(|e| format!("{:?}", e));
                        match res {
                            Err(e) => Err(format!("Error decoding JSON for {}: {}\n...from response: {}", addr, e, s)),
                            Ok(gds) => Ok(gds),
                        }
                    }
                }
            }
        }
    }
}

/// Takes a list of addresses (one per line) on stdin, retrieves the lat/lon for
/// each one and prints CSV results to stdout
#[tokio::main]
async fn main() {
    let mut wtr = csv::Writer::from_writer(io::stdout());

    let stdin = io::stdin();
    let opts = Opts::from_args();
    let api_key_raw = fs::read_to_string(opts.api_key_file).unwrap();
    let api_key = api_key_raw.trim();
    let delay = time::Duration::from_millis(1200);

    for line in stdin.lines() {
        let orig_addr = line.unwrap();
        let mut addr = orig_addr.clone();
        let mut resvec = get_latlons(api_key, &addr).await;
        let mut rlen = resvec.clone().map(|v| v.len());
        while rlen == Ok(0) || rlen.is_err() {
            match rlen {
                Err(ref e) => eprintln!("Got error: {}", e),
                Ok(_) => (),
            }
            addr = addr
// Dropping the zip code might be helpful in some situations, not sure though
//                .trim_end_matches(|c| char::is_digit(c, 10) || c == '-')
                .trim_start_matches(|c| c != ' ')
                .trim_start()
                .to_string();
            if addr.len() == 0 || addr.find(char::is_whitespace) == None {
                break;
            }
            eprintln!("Trimming and retrying new address: {}", addr);
            thread::sleep(delay);
            resvec = get_latlons(api_key, &addr).await;
            rlen = resvec.clone().map(|v| v.len());
        }
        match resvec {
            Err(e) => eprintln!("{}", e),
            Ok(gds) => {
                if gds.len() == 0 {
                    eprintln!("Got 0 results for {}", addr);
                }
                for gd in gds {
                    let agd = AddrGeoData {
                        address: orig_addr.clone(),
                        place_id: gd.place_id,
                        licence: gd.licence,
                        osm_type: gd.osm_type,
                        osm_id: gd.osm_id,
                        lat: gd.lat,
                        lon: gd.lon,
                        display_name: gd.display_name,
                        class: gd.class,
                        r#type: gd.r#type,
                        importance: gd.importance,
                    };
                    let _ = wtr.serialize(agd);
                    let _ = wtr.flush();
                }
            }
        }
        thread::sleep(delay);
    }

    let _ = wtr.flush();
}

