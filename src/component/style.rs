use askama::Template;
use std::env;

#[derive(Template)]
#[template(path = "style.json", escape = "none")]
pub struct Style {
    veloinfo_url: String,
}

pub async fn style() -> String {
    let veloinfo_url = env::var("VELOINFO_URL").unwrap();

    Style { veloinfo_url }.render().unwrap()
}
