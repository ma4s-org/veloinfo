use askama::Template;
use std::env;

#[derive(Template)]
#[template(path = "style.json", escape = "none")]
pub struct Style {
    martin_url: String,
    veloinfo_url: String,
}

pub async fn style() -> String {
    let martin_url = env::var("MARTIN_URL").unwrap();
    let veloinfo_url = env::var("VELOINFO_URL").unwrap();

    Style {
        martin_url,
        veloinfo_url,
    }
    .render()
    .unwrap()
}
