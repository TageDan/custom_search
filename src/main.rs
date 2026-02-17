use regex::Regex;
use rouille::{Response, router};
use serde::Deserialize;
use std::{collections::HashMap, error::Error, io::Read, path::PathBuf};
use toml::from_str;

/// simpler_custom_search is an application that provides a simpler way to create simpler search strings.
///
/// Using firefox you can create custom search strings under "about:preferences#search", "search shortcuts"
/// by putting %s inplace for the search string. This is great, but it only allows for one single parameter
/// (the entire search string) and there is no neat solution if you want more. This is what simpler_custom_search
/// solves. By adding new tables to the toml file you create a new local endpoint with it's own rules for
/// search string parsing and query string generation. A http redirection to the generated query url is then
/// sent as a response for the request to the endpoint.
///
/// The Regex crate is used for:
/// Regex capture groups are used to capture variables and regex expand is used to replace them in the query
/// generation template.

#[derive(Deserialize)]
struct CustomSearch {
    endpoint: String,
    parse_rule: String,
    gen_rule: String,
}

fn main() {
    let mut config_file = std::env::home_dir().unwrap();
    config_file.push(".config");
    config_file.push("custom_search");
    config_file.push("config.toml");

    rouille::start_server("localhost:8000", move |req| {
        router!(req,
                (GET) (/{endpoint: String}) => {
                    let query = req.get_param("q").unwrap_or(String::new());

                    match parse(endpoint,query, &config_file){
                        Ok(target) => Response::html(&format!("<script>window.location.href='{}'</script>", target)),
                        Err(err) => Response::html(&format!("{:?}", err)).with_status_code(404),
                    }
                },

            _ => Response::empty_404()
        )
    });
}

fn parse(ep: String, q: String, config_file: &PathBuf) -> Result<String, Box<dyn Error>> {
    let mut config_str = String::new();
    std::fs::File::open(config_file)?.read_to_string(&mut config_str)?;

    let enpoint_table: HashMap<String, Vec<CustomSearch>> = from_str(&config_str)?;
    let endpoints = enpoint_table
        .get("endpoint")
        .ok_or("no endpoints in config file")?;
    for CustomSearch {
        endpoint,
        parse_rule,
        gen_rule,
    } in endpoints
    {
        if endpoint == &ep {
            let regex = Regex::new(&parse_rule)?;
            if let Some(captures) = regex.captures(&q) {
                let mut result = String::new();
                println!("{:?}", captures);
                captures.expand(&gen_rule, &mut result);
                return Ok(result);
            }
            return Err("no captured groups".into());
        }
    }

    Err("no matching enpoint".into())
}
