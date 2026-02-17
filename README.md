# Custom Search

custom_search is an application that provides a simpler way to create simpler search strings.

Using firefox you can create custom search strings under `about:preferences#search`, `search shortcuts`
by putting `%s` inplace for the search string. This is great, but it only allows for one single parameter
(the entire search string) and there is no neat solution if you want more. This is what simpler_custom_search
solves. By adding new tables to the toml file you create a new local endpoint with it's own rules for
search string parsing and query string generation. A http redirection to the generated query url is then
sent as a response for the request to the endpoint.

The Regex crate is used for:
Regex capture groups are used to capture variables and regex expand is used to replace them in the query
generation template.

## Example
The following may be added to `~/.config/custom_search/config.toml`:
```toml
[[endpoint]]
endpoint = "translate"
parse_rule = '\[(..)\]\[(..)\](.+)'
gen_rule = "https://translate.google.com/?sl=$1&tl=$2&text=$3"
```
And a custom firefox search `http://localhost:8000/translate?q=%s`
With the keyword `tr`

Now translating english to swedish is as easy as searching `@tr [en][sv] hello`

## Installing

Install using cargo by running:
`cargo install --git https://github.com/TageDan/custom_search.git`
