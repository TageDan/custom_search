use regex::Regex;
use rhai::{Dynamic, Engine, EvalAltResult, Func};
use rouille::{Response, router};
use serde::Deserialize;
use std::{
    error::Error,
    fs::File,
    io::Read,
    net::{Ipv4Addr, SocketAddrV4},
    path::PathBuf,
};
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
///
/// The Rhai crate is used for providing scripting functionality for more complex query handling and suggestions api creation

#[derive(Deserialize, Clone)]
/// Struct representing the contents of the config file
struct Config {
    favicon: Option<String>,
    port: u16,
    endpoint: Vec<CustomSearch>,
}

#[derive(Deserialize, Clone)]
/// Struct representing a single enpoint in the config file
struct CustomSearch {
    endpoint: String,
    parse_rule: String,
    gen_rule: GenRule,
    suggestion_script: Option<PathBuf>,
}

#[derive(Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
/// generate with a simple template or using a rhai script
enum GenRule {
    Simple(String),
    Script(PathBuf),
}

fn main() -> Result<(), Box<dyn Error>> {
    create_missing_config()?;

    let conf = get_config()?;
    rouille::start_server(
        SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), conf.port),
        move |req| {
            // just returns a 404 since I need to return something from the log
            rouille::log(req, std::io::stdout(), Response::empty_404);

            router!(req,
                (GET) ["/favicon.ico"] => {
                    match get_favicon_ico() {
                        Some(p) => p,
                        None => Response::empty_404()
                    }
                },
                (GET) ["/{endpoint}", endpoint: String] => {
                    let query = req.get_param("q").unwrap_or_default();

                    match parse_query(endpoint,query){
                        Ok(target) => {Response::html(format!("<script>window.location.href='{target}'</script>"))},
                        Err(err) => Response::html(format!("{:?}", err)).with_status_code(404),
                    }
                },
                (GET) ["/{endpoint}/suggestions", endpoint: String] => {
                    let query = req.get_param("q").unwrap_or_default();
                    match parse_suggest(endpoint,query) {
                        Ok(list) => return Response::from_data("application/json",list),
                        Err(err) => Response::html(format!("{:?}", err)).with_status_code(404),
                    }
                },

                _ => Response::empty_404()
            )
        },
    );
}

/// Get the entire config from the config file
fn get_config() -> Result<Config, Box<dyn Error>> {
    let path = config_file_path().ok_or("no config file")?;

    let mut config_str = String::new();
    std::fs::File::open(path)?.read_to_string(&mut config_str)?;
    Ok(from_str(&config_str)?)
}

/// Get the file path for the config file (`~/.config/custom_search/config.toml`)
fn config_file_path() -> Option<PathBuf> {
    let mut config_file = std::env::home_dir()?;
    config_file.push(".config");
    config_file.push("custom_search");
    config_file.push("config.toml");
    Some(config_file)
}

/// create the config dir/file if it's missing
fn create_missing_config() -> Result<(), Box<dyn Error>> {
    let path = config_file_path().ok_or("no home directory")?;

    let dir_path = path.parent().ok_or("no config dir")?;

    // if config dir doesn't exists then create it
    if !std::path::Path::new(&dir_path).is_dir() {
        std::fs::create_dir_all(dir_path)?
    }

    // if config file doesn't exists then create it
    if !std::path::Path::new(&path).exists() {
        std::fs::File::create(&path)?;
    }

    Ok(())
}

/// Parse a query to an enpoint and generate a new query string to redirect to
fn parse_query(ep: String, q: String) -> Result<String, Box<dyn Error>> {
    let custom_search = get_enpoint_config(ep).ok_or("no config for endpoint")?;
    let regex = Regex::new(&custom_search.parse_rule)?;
    if let Some(captures) = regex.captures(&q) {
        let mut result = String::new();
        match custom_search.gen_rule {
            GenRule::Simple(string) => captures.expand(&string, &mut result),
            GenRule::Script(path) => result = run_query_script(path, captures)?,
        }
        return Ok(result);
    } else {
        let backup_regex = Regex::new(".*")?;
        let captures = backup_regex
            .captures(&custom_search.parse_rule)
            .ok_or("wtf")?;
        let result = run_query_script(
            custom_search
                .suggestion_script
                .ok_or("No suggestion_script")?,
            captures,
        )?;
        return Ok(result);
    }
}

fn parse_suggest(ep: String, q: String) -> Result<String, Box<dyn Error>> {
    let custom_search = get_enpoint_config(ep).ok_or("no config for endpoint")?;
    let regex = Regex::new(&custom_search.parse_rule)?;
    if let Some(captures) = regex.captures(&q) {
        let result = run_suggest_script(
            custom_search
                .suggestion_script
                .ok_or("No suggestion_script")?,
            captures,
        )?;
        return Ok(result);
    } else {
        let backup_regex = Regex::new(".*")?;
        let captures = backup_regex
            .captures(&custom_search.parse_rule)
            .ok_or("wtf")?;
        let result = run_suggest_script(
            custom_search
                .suggestion_script
                .ok_or("No suggestion_script")?,
            captures,
        )?;
        return Ok(result);
    }
}

/// run the rhai script specified by path using the capture groups as input and return the result.
fn run_query_script(
    path: PathBuf,
    captures: regex::Captures<'_>,
) -> Result<String, Box<dyn Error>> {
    let parts: Vec<String> = captures
        .iter()
        .map(|x| x.map(|m| m.as_str()).unwrap_or_default().to_owned())
        .collect();
    let func = get_query_script(&path)?;

    Ok(func(parts.into())?)
}

/// run the rhai script specified by path using the capture groups as input and return the result.
fn run_suggest_script(
    path: PathBuf,
    captures: regex::Captures<'_>,
) -> Result<String, Box<dyn Error>> {
    let parts: Vec<String> = captures
        .iter()
        .map(|x| x.map(|m| m.as_str()).unwrap_or_default().to_owned())
        .collect();
    let func = get_suggest_script(&path)?;

    Ok(func(parts.into())?)
}

/// Return the favicon as specified in the config file
fn get_favicon_ico() -> Option<Response> {
    let favicon_path = get_config().ok()?.favicon?;
    let file = File::open(favicon_path).ok()?;
    Some(Response::from_file("image/png", file))
}

/// Get config for a specific endpoint
fn get_enpoint_config(ep: String) -> Option<CustomSearch> {
    let config = get_config().ok()?;

    for custom_search in config.endpoint {
        if custom_search.endpoint == ep {
            return Some(custom_search.clone());
        }
    }
    None
}

type SuggestionFunc = Box<dyn Fn(Dynamic) -> Result<String, Box<EvalAltResult>>>;

/// get the rhai suggestion script specified by the path
fn get_suggest_script(file: &PathBuf) -> Result<SuggestionFunc, Box<dyn Error>> {
    let mut script_content = String::new();
    File::open(file)?.read_to_string(&mut script_content)?;
    let engine = Engine::new();
    let func = Func::<(Dynamic,), String>::create_from_script(engine, &script_content, "suggest")?;
    Ok(func)
}

type QueryFunc = Box<dyn Fn(Dynamic) -> Result<String, Box<EvalAltResult>>>;

/// get the rhai query generation script specified by the path
fn get_query_script(file: &PathBuf) -> Result<QueryFunc, Box<dyn Error>> {
    let mut script_content = String::new();
    File::open(file)?.read_to_string(&mut script_content)?;
    let engine = Engine::new();
    let func =
        Func::<(Dynamic,), String>::create_from_script(engine, &script_content, "create_query")?;
    Ok(func)
}
