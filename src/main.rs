use std::{sync::Arc, path::Path, fs};

use futures::future::join_all;

use anyhow::Result;
use anyhow::anyhow;
use atrium_api::agent::{AtpAgent, store::MemorySessionStore};
use atrium_xrpc_client::reqwest::ReqwestClient;

use promptly::prompt;
use serde::{Serialize, Deserialize};

use atrium_api::com::atproto::server::create_session::Input;
use atrium_api::agent::Session;

use base64::{engine, alphabet, Engine as _};

use base64::engine::general_purpose::URL_SAFE_NO_PAD as base64;

#[tokio::main]
async fn main() -> Result<()> {
    let api_id = std::fs::read_to_string("./api.id")?.trim().to_owned();
    let api_key = std::fs::read_to_string("./api.key")?.trim().to_owned();

    let mut agent = AtpAgent::new(
        ReqwestClient::new("https://bsky.social"),
        MemorySessionStore::default(),
    );

    let cookie_path = dirs::home_dir().unwrap().as_path().join(".local/bsky_auth");
    if try_restore_session(&mut agent, &cookie_path).await.is_err() {
        let cookie = authenticate(&mut agent, api_id, api_key).await?;
        cache_session(cookie, &cookie_path)?
    }

    eprintln!("Login successful");

    let mut cursor = None;
    loop {
        println!("Feching batch of 10");
        let get_timeline_params = atrium_api::app::bsky::feed::get_timeline::Parameters {
            algorithm: None,
            cursor: cursor.clone(),
            limit: Some(10),
        };
        let timeline = agent.api.app.bsky.feed.get_timeline(get_timeline_params).await?;
        cursor = timeline.cursor.clone();

        for post in timeline.feed.iter().map(|feed_item| &feed_item.post) {
            match &post.record {
                atrium_api::records::Record::AppBskyActorProfile(_) => todo!(),
                atrium_api::records::Record::AppBskyFeedGenerator(_) => todo!(),
                atrium_api::records::Record::AppBskyFeedLike(_) => todo!(),
                atrium_api::records::Record::AppBskyFeedPost(record) => print_post(&post, &record),
                atrium_api::records::Record::AppBskyFeedRepost(_) => todo!(),
                atrium_api::records::Record::AppBskyFeedThreadgate(_) => todo!(),
                atrium_api::records::Record::AppBskyGraphBlock(_) => todo!(),
                atrium_api::records::Record::AppBskyGraphFollow(_) => todo!(),
                atrium_api::records::Record::AppBskyGraphList(_) => todo!(),
                atrium_api::records::Record::AppBskyGraphListblock(_) => todo!(),
                atrium_api::records::Record::AppBskyGraphListitem(_) => todo!(),
            }

            let mut buf = String::new();
            std::io::stdin().read_line(&mut buf)?;

            match buf.as_str().trim() {
                "debug" => println!("{post:#?}"),
                "cursor" => println!("{cursor:?}"),
                _ => (),
            }
        }
    }

    Ok(())
}

fn print_post(
    post: &atrium_api::app::bsky::feed::defs::PostView, 
    record: &atrium_api::app::bsky::feed::post::Record) 
{
    if let Some(display_name) = &post.author.display_name {
        println!("{}", display_name)
    }
    println!("@{}\n", post.author.handle);
    println!("{}", record.text);
    if let Some(embed) = &post.embed {
        use atrium_api::app::bsky::feed::defs::PostViewEmbedEnum;
        match embed {
            PostViewEmbedEnum::AppBskyEmbedImagesView(image) => {
                for image in &image.images {
                    println!("\tIMAGE: {}", image.thumb);
                    if !image.alt.is_empty() {
                        println!("\t       {}", image.alt);
                    }
                }
            },
            PostViewEmbedEnum::AppBskyEmbedExternalView(ext) => {
                let ext = &ext.external;
                println!("\tEXTERNAL: {}", ext.title);
                println!("\t          {}", ext.description);
                println!("\t          {}", ext.thumb.as_deref().unwrap_or(""));
                println!("\t          {}", ext.uri);
            },
            PostViewEmbedEnum::AppBskyEmbedRecordView(_) => todo!(),
            PostViewEmbedEnum::AppBskyEmbedRecordWithMediaView(_) => todo!(),
        }
    }
    println!("Replies: {}\tLikes: {}\tReposts: {}",
        post.like_count.unwrap_or(0),
        post.reply_count.unwrap_or(0),
        post.repost_count.unwrap_or(0),
    );
    println!();
}

fn get_command() {
    let cmd: String = prompt("> ").unwrap();

}

async fn try_restore_session(
    agent: &mut AtpAgent<MemorySessionStore, ReqwestClient>, 
    session_cookie_path: &Path
) -> Result<()> {
    
    let baked_cookie = std::fs::read_to_string(&session_cookie_path)?;
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

    eprintln!("Logging in with cached session");

    agent.resume_session(old_session).await?;

    eprintln!("Cached session successfully validated");
    return Ok(())
}

async fn authenticate(
    agent: &mut AtpAgent<MemorySessionStore, ReqwestClient>,
    id: String,
    key: String,
) -> Result<SessionCookie> {
    eprintln!("Logging in as {id} ({key})");
    let session = agent.login(&id, &key).await?;
    eprintln!("App-password login successful.");

    let cookie = SessionCookie::from_session(&session);
    Ok(cookie)
}

fn cache_session(cookie: SessionCookie, path: &Path) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    fs::File::create(&path)?;
    fs::write(&path, cookie.to_string())?;
    Ok(())
}

struct BlackskyAuth {

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