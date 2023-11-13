use std::sync::Arc;

use futures::future::join_all;

use anyhow::Result;
use atrium_api::{client::AtpServiceClient, agent::AtpAgent};
use atrium_xrpc_client::reqwest::ReqwestClient;

use atrium_api::com::atproto::server::create_session::Input;

#[tokio::main]
async fn main() -> Result<()> {
    let api_id = "kotblini.bsky.social";
    let api_key = std::fs::read_to_string("./api.key")?;

    let agent = Arc::new(AtpAgent::new(ReqwestClient::new("https://bsky.social")));
    agent.login(&api_id, &api_key).await?;

    let actors = ["safety.bsky.app", "bsky.app"];
    let handles = actors
        .iter()
        .map(|&actor| {
            let agent = Arc::clone(&agent);
            tokio::spawn(async move {
                agent.api.app.bsky.actor
                    .get_profile(atrium_api::app::bsky::actor::get_profile::Parameters {
                        actor: actor.into(),
                    })
                    .await
            })
        })
        .collect::<Vec<_>>();

    let results = join_all(handles).await;

    for (actor, result) in actors.iter().zip(results) {
        println!("{actor}: {:#?}", result??);
    }

    Ok(())
}
