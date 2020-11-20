use std::
{
	borrow::Cow,
	sync::Arc,
	time::Duration,
};

use serenity::
{
	prelude::*,
	model::prelude::*,
	async_trait,
	client::bridge::gateway::{ShardId, ShardManager},
	framework::{
		StandardFramework,
		standard::{
			Args, CommandError, CommandResult,
			macros::{command, group, hook},
		},
	},
};
use tokio::{*, io::AsyncWriteExt};
use reqwest as http;
use anyhow::{anyhow, Context as _};

use tracing as log;
use tracing_subscriber;

use serde::{Serialize, Deserialize};
use serde_json;
use indexmap;

const CACHE_PATH: &str = "cache.json";
const CACHE_SIZE: usize = 128;


fn main() -> anyhow::Result<()>
{
	tracing_subscriber::fmt()
		.with_max_level(log::Level::INFO)
		.compact()
		.init();

	let mut rt = runtime::Builder::new()
		.basic_scheduler()
		.enable_all()
		.build()?;

	rt.block_on(run())?;
	rt.shutdown_timeout(time::Duration::from_secs(10));
	Ok(())
}

async fn run() -> anyhow::Result<()>
{
	// https://discordapp.com/api/oauth2/authorize?client_id=340869720775327744&permissions=67619904&scope=bot
	let token = std::env::var("CIRI_TOKEN").context("failed to read bot token. Is $CIRI_TOKEN set?")?;

	let mut client = Client::builder(&token)
		.event_handler(Handler)
		.framework(StandardFramework::new()
			.configure(|c| c.prefix("."))
			.before(before_hook)
			.after(after_hook)
			.group(&NETWORK_GROUP)
			.group(&PR0GRAMM_GROUP)
		)
		.await?;

	let cache = Cache::load_from_file(CACHE_PATH).await
		.unwrap_or_else(|err| {
			log::warn!("failed to load cache: {}", err);
			Default::default()
		});
	let cache_entries: u64 = cache.pr0_posts.iter().flat_map(|map| map.1).sum();
	if cache_entries > 0 {
		log::info!("loaded {} cache entries", cache_entries);
	}
	let save_notifier = cache.save_notifier.clone();

	let client_data = client.data.clone();
	{
		let mut state = client_data.write().await;
		state.insert::<ShardManagerContainer>(client.shard_manager.clone());
		state.insert::<Cache>(cache);
	}

	spawn(async move {
		loop {
			save_notifier.notified().await;
			let state = client_data.read().await;
			let cache = state.get::<Cache>().unwrap();
			log::debug!(size = cache.entries_count(), "saving cache");
			if let Err(err) = cache.save().await {
				log::error!("failed to save cache: {}", err);
			}
		}
	});

	log::info!("Connecting...");
	client.start().await
		.context("failed to start serenity client")
}

/*  _______    _________   _____________
   / ____/ |  / / ____/ | / /_  __/ ___/
  / __/  | | / / __/ /  |/ / / /  \__ \
 / /___  | |/ / /___/ /|  / / /  ___/ /
/_____/  |___/_____/_/ |_/ /_/  /____/
*/
struct Handler;

#[async_trait]
impl EventHandler for Handler
{
	async fn ready(&self, _ctx: Context, rdy: Ready)
	{
		log::info!("Connected! Serving {} guilds", rdy.guilds.len());
	}

	async fn guild_create(&self, _ctx: Context, gld: Guild, _is_new: bool)
	{
		log::info!("Joined {:?}", gld.name);
	}
}

/* __________  __  _____  ______    _   ______  _____
  / ____/ __ \/  |/  /  |/  /   |  / | / / __ \/ ___/
 / /   / / / / /|_/ / /|_/ / /| | /  |/ / / / /\__ \
/ /___/ /_/ / /  / / /  / / ___ |/ /|  / /_/ /___/ /
\____/\____/_/  /_/_/  /_/_/  |_/_/ |_/_____//____/
*/
#[hook]
async fn after_hook(ctx: &Context, msg: &Message, cmd: &str, res: CommandResult)
{
	//  Print out an error if it happened
	if let Err(why) = res {
		log::error!("failed: {}: {:?}", cmd, why);
		msg.channel_id.say(&ctx.http, format!("{}: failed: {}", msg.author.mention(), why))
			.await
			.expect("failed to send error reply");
	}
}

#[hook]
async fn before_hook(_ctx: &Context, msg: &Message, _cmd: &str) -> bool {
	log::info!("CMD {}: {}", msg.author.tag(), msg.content);
	true
}

#[group]
#[commands(ping, isup)]
struct Network;

#[command]
async fn ping(ctx: &Context, msg: &Message) -> CommandResult
{
	let mut reply = msg.reply(ctx, "Pong!").await?;

	let data = ctx.data.read().await;
	let shard_manager = data.get::<ShardManagerContainer>().ok_or(anyhow!("failed to get shard manager"))?;
	let manager = shard_manager.lock().await;
	let runners = manager.runners.lock().await;
	match runners.get(&ShardId(ctx.shard_id)).and_then(|runner| runner.latency)
	{
		None => log::warn!("no shards found!"),
		Some(latency) => {
			fn dur_fmt(dur: &Duration) -> f64
			{
				let integer = dur.as_secs();
				let decimal = dur.subsec_nanos() as u64;
				let mut exp = 10u64;
				while decimal >= exp { exp *= 10; }
				(integer as f64 + decimal as f64 / exp as f64) * 1000f64
			}
			let _ = reply.edit(ctx, |m| m.content(&format!("Pong! Latency: {:.3} ms", dur_fmt(&latency)))).await;
		},
	}
	Ok(())
}

struct ShardManagerContainer;
impl TypeMapKey for ShardManagerContainer {
	type Value = Arc<Mutex<ShardManager>>;
}

#[command]
async fn isup(ctx: &Context, msg: &Message, args: Args) -> CommandResult
{
	if !args.is_empty() {
		return Err(anyhow!("Missing URL").into());
	}

	let domain = args.rest();
	let mut reply = msg.channel_id.say(ctx, format!("checking {}", domain))
		.await
		.context("failed to reply")?;

	let url: Cow<str> = if domain.starts_with("http") {
		domain.into()
	} else {
		format!("http://{}", domain).into()
	};

	let res = match http::Client::builder()
		.timeout(Duration::from_secs(10))
		.build()
		.context("internal client error")?
		.head(url.as_ref())
		.send()
		.await
		.context("is not reponding")?
		.error_for_status()
			.map(|_| format!("is online"))
			.map_err(|err| err.status()
				.map(|s| format!("is online, but... {}", s))
				.unwrap_or(String::new()))
	{
		Err(s) => s,
		Ok(s) => s,
	};

	reply.edit(ctx, |m| m.content(format!("{}: {} {}", msg.author.mention(), domain, res)))
		.await
		.context("failed to reply")?;

	Ok(())
}


/*  ____  ____  ____  __________  ___    __  _____  ___
   / __ \/ __ \/ __ \/ ____/ __ \/   |  /  |/  /  |/  /
  / /_/ / /_/ / / / / / __/ /_/ / /| | / /|_/ / /|_/ /
 / ____/ _, _/ /_/ / /_/ / _, _/ ___ |/ /  / / /  / /
/_/   /_/ |_|\____/\____/_/ |_/_/  |_/_/  /_/_/  /_/
*/
#[group]
#[commands(pr0, kadse, wuffer, otten, ente, waschkadse)]
struct Pr0gramm;

#[command]
pub async fn kadse(ctx: &Context, msg: &Message, _args: Args) -> CommandResult
{
	pr0_fetch(ctx, msg, &vec!["kadse", "süßvieh", "awww"]).await
}

#[command]
async fn wuffer(ctx: &Context, msg: &Message, _args: Args) -> CommandResult
{
	pr0_fetch(ctx, msg, &vec!["bellkadse", "süßvieh", "awww"]).await
}

#[command]
async fn waschkadse(ctx: &Context, msg: &Message, _args: Args) -> CommandResult
{
	pr0_fetch(ctx, msg, &vec!["müllpanda", "awww"]).await
}

#[command]
async fn otten(ctx: &Context, msg: &Message, _args: Args) -> CommandResult
{
	pr0_fetch(ctx, msg, &vec!["otten", "awww"]).await
}

#[command]
async fn ente(ctx: &Context, msg: &Message, _args: Args) -> CommandResult
{
	pr0_fetch(ctx, msg, &vec!["ente", "gut", "alles", "gut"]).await
}

#[command]
async fn pr0(ctx: &Context, msg: &Message, args: Args) -> CommandResult
{
	let tags = args.message()
		.split_ascii_whitespace()
		.collect::<Vec<_>>();

	pr0_fetch(ctx, msg, &tags).await
}

async fn pr0_fetch(ctx: &Context, msg: &Message, args: &[&str]) -> CommandResult
{
	#[derive(Debug,Deserialize)]
	struct Image {
		id: u64,
		promoted: u64,
		image: String,
//		thumb: String,
//		created: u64,
		up: i64,
		down: i64,
		deleted: Option<u32>,
	};

	#[derive(Debug,Deserialize)]
	struct Res {
		items: Vec<Image>
	}

	fn file_is_video(f: &str) -> bool {
		f.ends_with(".webm") || f.ends_with(".mp4")
	}

	let tags = args.join(" ");

	log::debug!("searching for '{}'", &tags);
	let mut reply = msg.channel_id.say(&ctx, &format!("Searching for {}...", tags)).await?;
	let gid = msg.guild_id.unwrap_or_default().0;

	let client = http::Client::builder()
		.timeout(Duration::from_secs(10))
		.gzip(true)
		.build()?;

	let mut last_id: u64 = 0;
	let mut images: Vec<Image>;
	loop {
		let mut req = client.get("https://pr0gramm.com/api/items/get")
			.query(&[("promoted", "1")])
			.query(&[("tags", &tags)]);

		if last_id != 0 {
			req = req.query(&[("older", last_id)]);
		}

		images = req.send().await?
			.json::<Res>().await?
			.items;

		log::debug!("{} results", images.len());
		if let Some(img) = images.last()
		{
			last_id = img.promoted;
		} else {
			break;
		}

		images.retain(|img| img.deleted.is_none());
		{
			let state = ctx.data.read().await;
			if let Some(cache) = state.get::<Cache>()
			{
				images.retain(|img|
					cache.pr0_posts.get(&gid)
						.map(|set| !set.contains(&img.id)).unwrap_or(true));
			}
		}
		if !images.is_empty() { break; }
		log::debug!("{} retained", images.len());
	}

	images.sort_unstable_by(|a, b| (a.up - a.down).cmp(&(b.up - b.down)));

	let choosen = images.last().ok_or(CommandError::from("no unused images found"))?;

	log::info!("Posting {} - {} (u: {} d: {})", &choosen.id, &choosen.image, choosen.up, choosen.down);
	reply.edit(ctx, |m| {
			let sub = if file_is_video(&choosen.image) { "vid" } else { "img" };
			m.content(format!("{}: https://{}.pr0gramm.com/{}", msg.author.mention(), sub, choosen.image))
		})
		.await?;

	{
		let mut state = ctx.data.write().await;
		if let Some(cache) = state.get_mut::<Cache>()
		{
			log::debug!(choosen.id, "blacklisting entry");
			let set = cache.pr0_posts.entry(gid).or_default();
			set.insert(choosen.id);
			if set.len() > CACHE_SIZE {
				set.pop();
			}
			cache.save_notifier.notify();
		}
	}
	Ok(())
}

#[derive(Default, Deserialize, Serialize)]
struct Cache {
	pr0_posts: std::collections::HashMap<u64, indexmap::IndexSet<u64>>,
	#[serde(skip)]
	save_notifier: Arc<sync::Notify>,
}
impl Cache {
	async fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
		std::fs::File::open(path).context("failed to open cache file")
			.and_then(|f| serde_json::from_reader(f).context("failed to parse cache"))
	}

	fn entries_count(&self) -> u64 {
		self.pr0_posts.iter().flat_map(|map| map.1).sum()
	}

	async fn save(&self) -> anyhow::Result<()> {
		let json = serde_json::to_string(&self)
			.context("failed to serialize cache")?;

		fs::OpenOptions::new().write(true).create(true)
			.open(&CACHE_PATH)
			.await
			.context("failed to open cache file")?
			.write_all(json.as_bytes())
			.await
			.context("failed to write cache")
	}
}
impl TypeMapKey for Cache {
	type Value = Self;
}
