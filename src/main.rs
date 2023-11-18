use std::{sync::Arc, path::Path, fs};

use futures::future::join_all;

use anyhow::Result;
use atrium_api::{client::AtpServiceClient, agent::AtpAgent};
use atrium_xrpc_client::reqwest::ReqwestClient;

use serde::{Serialize, Deserialize};

use atrium_api::com::atproto::server::create_session::Input;
use atrium_api::agent::Session;

use base64::{engine, alphabet, Engine as _};

use base64::engine::general_purpose::URL_SAFE_NO_PAD as base64;

#[tokio::main]
async fn main() -> Result<()> {
    let api_id = std::fs::read_to_string("./api.id")?.trim().to_owned();
    let api_key = std::fs::read_to_string("./api.key")?.trim().to_owned();

    let agent = authenticate(api_id, api_key).await?;

    Ok(())
}

async fn authenticate(
    id: String,
    key: String,
) -> Result<AtpAgent<ReqwestClient>> {

    let agent = AtpAgent::new(ReqwestClient::new("https://bsky.social"));
    
    let cookie_path = dirs::home_dir().unwrap().as_path().join(".local/bsky_auth");

    if let Ok(baked_cookie) = std::fs::read_to_string(&cookie_path) {

        let session_cookie = SessionCookie::from_str(&baked_cookie)?;

        let old_session = Session {
            access_jwt: session_cookie.access,
            did: session_cookie.did,
            did_doc: None,
            email: None,
            email_confirmed: None,
            handle: String::new(),
            refresh_jwt: session_cookie.refresh,
        };

        println!("Logging in with cached session");
        println!("Old session: {old_session:#?}");

        match agent.resume_session(old_session).await {
            Ok(_) => {
                eprintln!("Cached session successfully validated");
                let sesh = agent.get_session().await.unwrap();
                println!("Validated session: {sesh:#?}");
                return Ok(agent)
            },
            Err(e) => {
                eprintln!("Error resuming session: {e}");
                eprintln!("Trying reauthentication");
            }
        }
    }

    println!("Logging in as {id} ({key})");
    let session = agent.login(&id, &key).await?;
    println!("App-password login successful.");

    print!("Caching session");
    let cookie = SessionCookie::from_session(&session).to_string();
    println!(" to {}", &cookie_path.display());
    fs::create_dir_all(cookie_path.parent().unwrap())?;
    fs::File::create(&cookie_path)?;
    fs::write(&cookie_path, cookie)?;

    Ok(agent)
}

#[derive(Serialize, Deserialize)]
struct SessionCookie {
    access: String,
    did: String,
    refresh: String,
}

impl SessionCookie {
    fn from_session(session: &Session) -> Self {
        SessionCookie {
            access: session.access_jwt.clone(),
            did: session.did.clone(),
            refresh: session.refresh_jwt.clone(),
        }
    }

    fn from_str(input: &str) -> Result<Self> {
        let json_raw = base64.decode(input)?;
        let json = std::str::from_utf8(&json_raw)?;
        let session = serde_json::from_str(json)?;
        Ok(session)
    }

    fn to_string(&self) -> String {
        let json = serde_json::to_string(&self).unwrap();
        let b64_str = base64.encode(json);
        b64_str
    }
}