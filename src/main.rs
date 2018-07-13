
#[macro_use]
extern crate log;
extern crate fern;
extern crate chrono;

extern crate serenity;
extern crate typemap;

extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

extern crate reqwest;
extern crate rand;
//extern crate gifski;

use serenity::prelude::*;
use serenity::model::prelude::*;

use serenity::client::bridge::gateway::{ShardId, ShardManager};
use serenity::framework::standard::*;

use fern::colors::{Color, ColoredLevelConfig};

use std::sync::Arc;
use std::{env, fs};

mod cache;

const CACHE_PATH: &'static str = "ciri.json";
const CACHE_SIZE: usize = 128;


struct Handler;

struct ShardManagerContainer;
impl typemap::Key for ShardManagerContainer {
	type Value = Arc<Mutex<ShardManager>>;
}

struct Pr0List
{
	pub blacklist: cache::Queue<u64>
}

struct Pr0ListKey;
impl typemap::Key for Pr0ListKey {
	type Value = Pr0List;
}


fn main() 
{
	let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        // we actually don't need to specify the color for debug and info, they are white by default
        .info(Color::White)
        .debug(Color::White)
        // depending on the terminals color scheme, this is the same as the background color
        .trace(Color::BrightBlack);

    // configure colors for the name of the level.
    // since almost all of them are the some as the color for the whole line, we just clone
    // `colors_line` and overwrite our changes
    let colors_level = colors_line.clone()
        .info(Color::Green);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{color_line}[{date}][{target}][{level}{color_line}] {message}\x1B[0m",
                color_line = format_args!("\x1B[{}m", colors_line.get_color(&record.level()).to_fg_str()),
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = colors_level.color(record.level()),
                message = message,
            ));
        })
        // set the default log level. to filter out verbose log messages from dependencies, set
        // this to Warn and overwrite the log level for your crate.
        .level(log::LevelFilter::Info)
        // change log levels for individual modules. Note: This looks for the record's target
        // field which defaults to the module path but can be overwritten with the `target`
        // parameter:
        // `info!(target="special_target", "This log message is about special_target");`
        .level_for("ciri", log::LevelFilter::Trace)
        // output to stdout
        .chain(std::io::stdout())
        .apply().unwrap();


	let mut blacklist = fs::OpenOptions::new().read(true)
		.open(&CACHE_PATH).map_err(|_| ())
		.and_then(|f| serde_json::from_reader(f).map_err(|_| ()))
		.unwrap_or(cache::Queue::new());

	blacklist.reserve(CACHE_SIZE);
	if blacklist.len() > 0
	{
		blacklist.optimize();

		info!("Loaded {} used entries", blacklist.len());
//		debug!("{}", blacklist);
	}

	info!("Connecting...");

	// https://discordapp.com/api/oauth2/authorize?client_id=340869720775327744&permissions=67619904&scope=bot
	let mut client = Client::new(&env::var("DC_TOKEN").expect("TOKEN missing"), Handler).unwrap();
	{
		let mut data = client.data.lock();
		data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
		data.insert::<Pr0ListKey>(Pr0List { blacklist });
	}

	let fw = StandardFramework::new()
		.configure(|c| c
			.on_mention(true)
			.prefix(".")
		)
		.before(|_ctx, msg, _cmd|{
			info!("CMD {}: {}", msg.author.tag(), msg.content);
			true
		})
		.after(|_ctx, _msg, cmd_name, error| {
			//  Print out an error if it happened
			if let Err(why) = error {
				error!("failed: {}: {:?}", cmd_name, why);
			}
		})
/*		.on_dispatch_error(|_, msg, error| {
				match error {
					NotEnoughArguments { min, given } => {
						let s = format!("Need {} arguments, but only got {}.", min, given);

						let _ = msg.channel_id.say(&s);
					},
					TooManyArguments { max, given } => {
						let s = format!("Max arguments allowed is {}, but got {}.", max, given);

						let _ = msg.channel_id.say(&s);
					},
					_ => println!("Unhandled dispatch error."),
				}
		})
*/
		.command("ping", |c| c.exec(ping))
		.command("pr0", |c| c.exec(pr0))
		.command("kadse", |c| c.exec(kadse))
		.command("otten", |c| c.exec(otten))
		.command("waschkadse", |c| c.exec(waschkadse))
		.command("ente", |c| c.exec(ente));


	client.with_framework(fw);

	if let Err(why) = client.start() {
		error!("Client error: {:?}", why);
	}
}

/*  _______    _________   _____________
   / ____/ |  / / ____/ | / /_  __/ ___/
  / __/  | | / / __/ /  |/ / / /  \__ \
 / /___  | |/ / /___/ /|  / / /  ___/ /
/_____/  |___/_____/_/ |_/ /_/  /____/
*/
impl EventHandler for Handler
{
	fn ready(&self, _ctx: Context, rdy: Ready)
	{
		info!("Connected! {} guilds to serve", rdy.guilds.len());
	}

	fn guild_create(&self, _ctx: Context, gld: Guild, _: bool)
	{
		info!("Joined {:?}", gld.name);
		gld.edit_nickname(Some("ISIS")).ok();
	}
}


/* __________  __  _____  ______    _   ______  _____
  / ____/ __ \/  |/  /  |/  /   |  / | / / __ \/ ___/
 / /   / / / / /|_/ / /|_/ / /| | /  |/ / / / /\__ \
/ /___/ /_/ / /  / / /  / / ___ |/ /|  / /_/ /___/ /
\____/\____/_/  /_/_/  /_/_/  |_/_/ |_/_____//____/
*/

// helper
fn fail<T: std::fmt::Display>(err: T) -> Result<(),CommandError>
{
	return Err(CommandError::from(err));
}

fn ping(ctx: &mut Context, msg: &Message, _args: Args) -> Result<(),CommandError>
{
	fn int2float_concat(integer: u64, decimal: u64) -> f64
	{
		let mut exp = 10u64;
		while decimal >= exp { exp *= 10; }
		integer as f64 + decimal as f64 / exp as f64
	}

	let chan = msg.channel_id;
	let mut reply = match chan.say("Pong!") {
		Err(e) => return fail(format!("failed to reply: {}", e)),
		Ok(v) => v
	};

	let latency =
	{
		let data = ctx.data.lock();
		let shard_manager = match data.get::<ShardManagerContainer>() {
			Some(v) => v,
			None => {
				error!("lat sh");
				return fail(&"failed to get shard manager");	
			} 
		};

		let manager = shard_manager.lock();
		let runners = manager.runners.lock();

		runners.get(&ShardId(ctx.shard_id))
			.and_then(|runner| runner.latency) 
			.map(|s| format!("{:.3} ms", int2float_concat(s.as_secs(), s.subsec_nanos() as u64) * 1000f64))
	};


	if latency.is_none() 
	{ 
		error!("lat det");
		return fail("Latency detection failed"); 
	}

	if let Err(e) = reply.edit(|m| m.content(&format!("Pong! Latency: {}", latency.unwrap())))
	{
		error!("lat rep");
		return fail(format!("Latency reply failed: {}", e));
	}

	Ok(())
}

fn kadse(ctx: &mut Context, msg: &Message, _args: Args) -> Result<(),CommandError>
{
	pr0_fetch(ctx, msg, &vec!["kadse", "süßvieh"])
}

fn waschkadse(ctx: &mut Context, msg: &Message, _args: Args) -> Result<(),CommandError>
{
	pr0_fetch(ctx, msg, &vec!["müllpanda", "awww"])
}

fn otten(ctx: &mut Context, msg: &Message, _args: Args) -> Result<(),CommandError>
{
	pr0_fetch(ctx, msg, &vec!["otten", "awww"])
}

fn ente(ctx: &mut Context, msg: &Message, _args: Args) -> Result<(),CommandError>
{
	pr0_fetch(ctx, msg, &vec!["ente", "gut", "alles", "gut"])
}

fn pr0(ctx: &mut Context, msg: &Message, args: Args) -> Result<(),CommandError>
{
	pr0_fetch(ctx, msg, &(&args).split_whitespace().collect::<Vec<_>>())
}

fn pr0_fetch(ctx: &mut Context, msg: &Message, args: &[&str]) -> Result<(),CommandError>
{
	#[derive(Debug,Deserialize)]
	struct Image {
		id: u64,
		image: String,
		thumb: String,
		created: u64,
		up: i64,
		down: i64,

	};
	#[derive(Debug,Deserialize)]
	struct Res {
		items: Vec<Image>
	}

	fn file_is_video(f: &str) -> bool {
	    f.ends_with(".webm") || f.ends_with(".mp4")
	}

	let mut reply = msg.channel_id.say(&format!("Searching for {}...", args[0]))?;

	let res = reqwest::get(&format!("https://pr0gramm.com/api/items/get?flags=9&promoted=1&tags={}", args.join("+")))
	                  .map(|mut r| r.json::<Res>());
	let mut images = match res {
		Err(e) => return fail(format!("kadse fetch failed: {}", e)),
		Ok(v) => match v {
			Err(e) => return fail(format!("kadse parse failed: {}", e)),
			Ok(v) => v.items
		}
	};

	{
		let data = ctx.data.lock();
		if let Some(cat_list) = data.get::<Pr0ListKey>()
		{
			images.retain(|img| !( // <--
				cat_list.blacklist.contains(&img.id)
//				|| file_is_video(&img.image)
			))
		}
	}
	images.sort_unstable_by(|a, b| (a.up - a.down).cmp(&(b.up - b.down)));

	use rand::Rng;

	let choosen = &rand::thread_rng().choose(&images).ok_or(CommandError::from("no image found"))?;

	info!("Posting {}", &choosen.image);
	{
		let mut data = ctx.data.lock();
		if let Some(cat_list) = data.get_mut::<Pr0ListKey>()
		{
			cat_list.blacklist.push(choosen.id);
			if let Ok(f) = fs::OpenOptions::new().write(true).create(true).open(&CACHE_PATH)
			{
				let res = serde_json::to_writer(f, &cat_list.blacklist);
				if res.is_err()
				{
					warn!("failed to save cache: {}", res.unwrap_err());
				}
			}
		}
	}

	reply.edit(|m|
		{
			let c = &choosen.image;
			let url = 
			{
				if file_is_video(&c)
				{
					format!("https://vid.pr0gramm.com/{}", &c)
				} else {
				    format!("https://img.pr0gramm.com/{}", &c)
				}
			};

			m.content(format!("{}: {}", msg.author.mention(), url))
		}
/*		.embed(|e| 
		{
			let e = e.colour(0xee4d2e);
			let c = &choosen.image;
			if c.ends_with(".webm") || c.ends_with(".mp4")
			{ 
				e.title(&choosen.image)
					.url(&format!("https://vid.pr0gramm.com/{}", c))
					.thumbnail(&format!("https://thumb.pr0gramm.com/{}", &choosen.thumb)) 
			}
			else { e.image(&format!("https://img.pr0gramm.com/{}", &c)) }
			
		})
*/	).map_err(|e| CommandError::from(&format!("failed to reply: {}", e)))
}
