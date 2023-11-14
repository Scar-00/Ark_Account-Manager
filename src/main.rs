use std::collections::HashSet;
use std::io::Write;

use serde::{Deserialize, Serialize};

use serenity::async_trait;
use serenity::client::bridge::gateway::{ShardId, ShardManager};
use serenity::framework::standard::buckets::{LimitedFor, RevertBucket};
use serenity::framework::standard::macros::{check, command, group, help, hook};
use serenity::framework::standard::{
    help_commands,
    Args,
    CommandGroup,
    CommandOptions,
    CommandResult,
    DispatchError,
    HelpOptions,
    Reason,
    StandardFramework,
};
use serenity::http::Http;
use serenity::model::channel::{Channel, Message};
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::UserId;
use serenity::model::user::User;
use serenity::model::permissions::Permissions;
use serenity::utils::MessageBuilder;
use serenity::prelude::*;
use tokio::sync::RwLock;
use std::sync::Arc;

struct AccountStorage {
    all: Vec<Account>,
    online: Vec<Account>,
    waiting: Vec<User>,
}

struct AccountsStorage;
impl TypeMapKey for AccountsStorage {
    type Value = Arc<RwLock<AccountStorage>>;
}

#[group]
#[commands(info, wait, log_on, log_off)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    /*async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            let channel = match msg.channel_id.to_channel(&ctx).await {
                Ok(channel) => channel,
                Err(why) => {
                    println!("Error getting channel: {:?}", why);

                    return;
                },
            };

            // The message builder allows for creating a message by
            // mentioning users dynamically, pushing "safe" versions of
            // content (such as bolding normalized content), displaying
            // emojis, and more.
            let response = MessageBuilder::new()
                .push("User ")
                .push_bold_safe(&msg.author.name)
                .push(" used the 'ping' command in the ")
                .mention(&channel)
                .push(" channel")
                .build();

            if let Err(why) = msg.channel_id.say(&ctx.http, &response).await {
                println!("Error sending message: {:?}", why);
            }
        }
    }*/

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} booted up successfully", ready.user.name);
    }
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!("Got command '{}' by user '{}'", command_name, msg.author.name);
    true // if `before` returns false, command processing doesn't happen.
}

#[hook]
async fn after(_ctx: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[hook]
async fn normal_message(ctx: &Context, msg: &Message) {
}

#[hook]
async fn delay_action(ctx: &Context, msg: &Message) {
    // You may want to handle a Discord rate limit if this fails.
    let _ = msg.react(ctx, '⏱').await;
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _command_name: &str) {
    if let DispatchError::Ratelimited(info) = error {
        // We notify them only once.
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(&ctx.http, &format!("Try this again in {} seconds.", info.as_secs()))
                .await;
        }
    }
}

async fn display_account_status(ctx: &Context) -> String {
    let accounts = {
        let data_read = ctx.data.read().await;
        data_read.get::<AccountsStorage>().expect("Expected AvailableAccounts in TypeMap.").clone()
    };
    let accounts = accounts.write().await;

    let mut builder = MessageBuilder::new();
        builder.push("# All accounts\n")
        .push_bold_line_safe("Online: ");
        {
            for acc in accounts.online.iter() {
                builder.push(format!("- {}\n", &acc.name));
            }
        }
        builder.push_bold_line("Offline:");
        {
            for acc in &accounts.all {
                if !accounts.online.contains(&acc) {
                    builder.push(format!("- {}\n", acc.name));
                }
            }
        }
    let message = builder.build();
    return message;
}

#[command]
async fn info(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;
    let mut messages = msg.channel_id.messages(&ctx.http, |b| {
        b
    }).await?;
    let message = display_account_status(ctx).await;
    if messages.len() < 1 {
        msg.channel_id.say(&ctx.http, message).await?;
    }else {
        messages[0].edit(&ctx.http, |msg| {
            msg.content(message)
        }).await?;
    }
    return Ok(());
}


#[command]
async fn wait(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;

    let accounts = {
        let data_read = ctx.data.read().await;
        data_read.get::<AccountsStorage>().expect("Expected AvailableAccounts in TypeMap.").clone()
    };
    let mut accounts = accounts.write().await;

    if accounts.online.len() == accounts.all.len() {
        msg.author.dm(&ctx.http, |user| {
            user.content("You will get notified as soon as an account is avaiable");
            user
        }).await?;

        accounts.waiting.push(msg.author.clone());
    }else {
        let mut avial_accs: Vec<Account> = Vec::new();
        for i in accounts.online.len()..accounts.all.len() {
            avial_accs.push(accounts.all[i].clone());
        }

        msg.author.dm(&ctx.http, |user| {
            let mut builder = MessageBuilder::new();
            builder.push("There are accounts(s) avaiable\n")
            .push_bold_line_safe("Available accounts:");
            for acc in avial_accs {
                builder.push(format!("- {}\n", acc.name));
            }
            let message = builder.build();
            user.content(&message);
            user
        }).await?;
    }

    return Ok(());
}

#[command]
async fn log_on(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;

    let accounts = {
        let data_read = ctx.data.read().await;
        data_read.get::<AccountsStorage>().expect("Expected AvailableAccounts in TypeMap.").clone()
    };
    let mut accounts = accounts.write().await;

    if let Ok(acc) = args.single::<String>() {
        if !accounts.all.contains(&Account{ name: acc.clone() }) {
            msg.author.dm(&ctx.http, |dm|{
                dm.content(&format!("Account `{}` is not recognised", acc))
            }).await?;
        }
        if accounts.online.contains(&Account{ name: acc.clone() }) {
            msg.author.dm(&ctx.http, |dm|{
                dm.content(&format!("Account `{}` is already in use", acc))
            }).await?;
        }

        accounts.online.push(Account{ name: acc });
    }
    return Ok(());
}

#[command]
async fn log_off(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    msg.delete(&ctx.http).await?;

    let accounts = {
        let data_read = ctx.data.read().await;
        data_read.get::<AccountsStorage>().expect("Expected AvailableAccounts in TypeMap.").clone()
    };
    let mut accounts = accounts.write().await;

    if let Ok(acc) = args.single::<String>() {
        if !accounts.all.contains(&Account{ name: acc.clone() }) {
            msg.author.dm(&ctx.http, |dm|{
                dm.content(&format!("Account `{}` is not recognised", acc))
            }).await?;
        }
        if !accounts.online.contains(&Account{ name: acc.clone() }) {
            msg.author.dm(&ctx.http, |dm|{
                dm.content(&format!("Account `{}` is already logged of", acc))
            }).await?;
        }

        if let Some(index) = accounts.online.iter().position(|e| e.name == acc) {
            accounts.online.remove(index);
        }
        if accounts.waiting.len() > 0 {
        let user = accounts.waiting.remove(0);
            user.dm(&ctx.http, |u| {
                u.content(&format!("An account has been freed up, you can now use the accout `{}`", acc))
            }).await?;
        }
    }

    return Ok(());
}

#[derive(Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
struct Account {
    name: String,
}


#[derive(Serialize, Deserialize, Debug)]
struct Accounts {
    accounts: Vec<Account>,
}

#[tokio::main]
async fn main() {
    let accounts = std::fs::read_to_string("accounts.json").unwrap();

    let accs: Accounts = serde_json::from_str(&accounts).unwrap();

    let token_file = "token.txt";
    let token = match std::fs::read_to_string(token_file) {
        Ok(t) => t,
        Err(_) => {
            println!("[ERROR]: failed to read `{}`", token_file);
            std::process::exit(1);
        }
    };

    let http = Http::new(&token);

    let (owners, bot_id) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        },
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c
        .with_whitespace(true)
        .on_mention(Some(bot_id))
        .prefix("/")
        .delimiters(vec![", ", ",", " "])
        .owners(owners))
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .normal_message(normal_message)
        .on_dispatch_error(dispatch_error)
        .group(&GENERAL_GROUP);

    let mut client = match Client::builder(token, GatewayIntents::all()).event_handler(Handler).framework(framework).await {
        Ok(c) => c,
        Err(err) => {
            println!("[ERROR]: `{}`", err);
            std::process::exit(1);
        }
    };

    {
        let mut data = client.data.write().await;
        data.insert::<AccountsStorage>(Arc::new(RwLock::new(AccountStorage{
            all: accs.accounts,
            online: Vec::new(),
            waiting: Vec::new(),
        })));
    }

    if let Err(err) = client.start().await {
        println!("[ERROR]: `{}`", err);
        std::process::exit(1);
    }
}
