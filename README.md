# JellyfinPulga

Media server management toolkit and community plugin for [Jellyfin](https://jellyfin.org).

## Features

### Jellyfin Plugin (C#/.NET)
Installs directly into Jellyfin — all features accessible from the Jellyfin web UI.

- **Per-movie chat rooms** — discuss any movie with other users on your server
- **Private messaging** — send direct messages to other Jellyfin users
- **Content reporting** — report corrupted files, wrong content, bad quality, duplicates
- **User blocking** — block users from messaging you
- **Admin review** — admins can update report status (open/reviewed/resolved/dismissed)

### CLI Tool (Rust)
Runs from your laptop, connects to the server via SSH.

- **Junk file scanner** — detect torrent site ads, promo images, metadata junk (.nfo, .txt, .html, .url)
- **Junk cleaner** — delete detected junk files (skips Jellyfin trickplay data)
- **Corruption checker** — validate media files using ffprobe, flag broken ones
- **Jellyfin tagging** — auto-tag corrupted items with `needs-review` in Jellyfin
- **Review workflow** — list flagged items, mark as reviewed, export TSV
- **Deploy command** — deploy the web UI to your server via SSH + Docker
- **Version checking** — validates Jellyfin API compatibility on startup
- **Retry logic** — 3 retries with backoff on connection failure

## Install the Plugin

### From repository (recommended)
1. In Jellyfin, go to **Dashboard > Plugins > Repositories**
2. Add repository URL:
   ```
   https://raw.githubusercontent.com/pulgamecanica/JellyfinPulga/main/manifest.json
   ```
3. Go to **Catalog**, find **JellyfinPulga**, and install it
4. Restart Jellyfin

### Manual install
1. Download `jellyfin-pulga-plugin.zip` from [Releases](https://github.com/pulgamecanica/JellyfinPulga/releases)
2. Extract to `/var/lib/jellyfin/plugins/JellyfinPulga_0.1.0.0/`
3. Restart Jellyfin

## Install the CLI Tool

### From release
```bash
# Download from GitHub releases
tar xzf jellyfin-pulga-linux-x86_64.tar.gz
cp config.example.toml config.toml
# Edit config.toml with your Jellyfin URL, API key, and SSH settings
```

### From source
```bash
git clone https://github.com/pulgamecanica/JellyfinPulga.git
cd JellyfinPulga
cargo build --release
cp config.example.toml config.toml
```

## CLI Usage

```bash
# Scan for junk files (dry run)
jellyfin-pulga scan

# Delete junk files
jellyfin-pulga clean

# Check media files for corruption
jellyfin-pulga check --tag          # also tags corrupted items in Jellyfin
jellyfin-pulga check --output results.json

# View/manage flagged items
jellyfin-pulga flagged
jellyfin-pulga review <item-id>
jellyfin-pulga export -o flagged.tsv

# Deploy web UI to server
jellyfin-pulga deploy up
jellyfin-pulga deploy status
jellyfin-pulga deploy logs

# Other
jellyfin-pulga users               # list Jellyfin users
jellyfin-pulga reports              # list content reports
jellyfin-pulga -v <command>         # verbose mode
jellyfin-pulga serve --port 3001    # run web UI locally
```

## Configuration

```toml
[jellyfin]
url = "http://192.168.1.37:8096"
api_key = "your-api-key"

[media]
paths = ["/srv/media/movies", "/srv/media/series"]
ffprobe_path = "/usr/lib/jellyfin-ffmpeg/ffprobe"

[server]
host = "0.0.0.0"
port = 3000

[execution]
mode = "ssh"    # "ssh" or "local"

[execution.ssh]
host = "192.168.1.37"
user = "pulgamecanica-serv"
port = 22
```

## Plugin API Endpoints

All endpoints require Jellyfin authentication.

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/Pulga/Chat/{roomId}/Messages` | Get chat messages for a movie |
| POST | `/Pulga/Chat/{roomId}/Send` | Send a chat message |
| GET | `/Pulga/Messages/Conversations` | List PM conversations |
| GET | `/Pulga/Messages/{userId}` | Get messages with a user |
| POST | `/Pulga/Messages/{userId}/Send` | Send a private message |
| POST | `/Pulga/Messages/{userId}/Read` | Mark messages as read |
| POST | `/Pulga/Messages/Block/{userId}` | Block a user |
| POST | `/Pulga/Messages/Unblock/{userId}` | Unblock a user |
| GET | `/Pulga/Reports` | List content reports |
| POST | `/Pulga/Reports` | Create a report |
| POST | `/Pulga/Reports/{id}/Status` | Update report status (admin) |
| GET | `/Pulga/Reports/Export` | Export reports as TSV |

## Requirements

- Jellyfin 10.9+
- For CLI: Rust 1.70+ (build) or download pre-built binary
- For plugin: .NET 9 SDK (build) or install from repository

## License

MIT
