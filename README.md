# repeat-bot

A Discord bot that detects when someone posts a link that's already been shared in the channel.

## Smart URL normalization

The bot doesn't just do string comparison — it normalizes URLs before checking for duplicates:

- **Tracking parameter removal** — strips `?s=`, `?utm_*`, `?fbclid`, `?si`, and dozens of other tracking/share junk params
- **Domain unification** — `twitter.com`, `vxtwitter.com`, `fxtwitter.com`, `nitter.net` → `x.com`; `old.reddit.com`, `vxreddit.com` → `reddit.com`; `youtu.be` → `youtube.com`; etc.
- **YouTube short links** — `youtu.be/dQw4w9WgXcQ` matches `youtube.com/watch?v=dQw4w9WgXcQ`
- **Protocol normalization** — `http://` → `https://`
- **Cosmetic normalization** — strips `www.`, trailing slashes, URL fragments

## Setup

1. Create a Discord bot at the [Developer Portal](https://discord.com/developers/applications)
2. Enable the **Message Content Intent** in Bot settings
3. Invite the bot with the `Send Messages` and `Read Message History` permissions

```sh
cp .env.example .env
# Edit .env with your bot token
```

## Run

```sh
DISCORD_TOKEN=your_token cargo run
```

## Test

```sh
cargo test
```

## How it works

- Listens to all guild messages (ignores bots)
- Extracts URLs from message text
- Normalizes each URL to a canonical form
- Checks SQLite for any prior occurrence in the same channel
- If a match is found (from a different user), replies with a repost notice
- All links are recorded for future lookups
