# sidefeed (WIP)

Simple aggregation service that takes links, as URLs and converts said URLs into a combined feed.
This allows for unified feeds and works agnostic to frontends.

## frontends for project

- example, [svelte-feeds](https://github.com/swarnimarun/svelte-feeds)
- example, [dioxus-feeds](https://github.com/swarnimarun/dioxus-feeds)

## Features

- Support for RSS(atom, v1 & v2) urls.
- Provide a HTTP REST API.

## Roadmap

- Support mass URL imports.
- Support OPML imports.
- Provide live-connection for feed updates(server-sent events).
- Add caching for feeds to db.
- Add search in cached feeds support, with sqlite full text search.
- Support fediverse, and aggregation on hashtags.
- Support websockets for api.
- Add web-scraping for feed aggregation.
- Support profiles for building multiple feeds.
- Support auth(maybe?).

## Credits

- rss (rust library for rss),
- actix & tokio,
- other crates I am using(left me know if you want your library explicitly named).

## Author

Swarnim Arun<br/>
Email Me At: swarnimarun11"at"gmail"dot"com
