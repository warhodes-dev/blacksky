use std::sync::Arc;

use futures::future::join_all;

use anyhow::Result;
use atrium_api::{client::AtpServiceClient, agent::AtpAgent};
use atrium_xrpc_client::reqwest::ReqwestClient;

use serde::ser::Serialize;

use atrium_api::com::atproto::server::create_session::Input;
use atrium_api::agent::Session;

#[tokio::main]
async fn main() -> Result<()> {
    let api_id = std::fs::read_to_string("./api.id")?.trim().to_owned();
    let api_key = std::fs::read_to_string("./api.key")?.trim().to_owned();

    let agent = authenticate(api_id, api_key).await?;

    println!("{:#?}", agent.get_session().await);

    Ok(())
}

async fn authenticate(
    id: String,
    key: String,
) -> Result<AtpAgent<ReqwestClient>> {

    let agent = AtpAgent::new(ReqwestClient::new("https://bsky.social"));
    
    if let Ok(old_session_json) = std::fs::read_to_string("./session.json") {
        println!("Logging in with cached session");
        let old_session: Session = serde_json::from_str(&old_session_json)?;
        match agent.resume_session(old_session).await {
            Ok(_) => {
                eprintln!("Cached session successfully validated");
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
    let session_json = serde_json::to_string(&session)?;
    println!("App-password login successful. Caching session to ./session.json");
    std::fs::write("./session.json", session_json)?;

    Ok(agent)
}
