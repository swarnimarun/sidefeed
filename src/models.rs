use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// TODO(swarnim): add support for profiles and users probably,
// then add support for UUIDs as well.

pub type FeedSourceId = usize;
/// [`Feed`]: is a table with a metadata with id pointing a url.
/// ## why?
/// allows for adding metadata for urls
pub type Feed = HashMap<FeedSourceId, FeedSource>;
#[derive(Debug, Serialize, Deserialize)]
pub struct FeedSource {
    /// feed name
    pub name: String,
    /// id to urls
    pub url: (String, UrlType),
    /// store time for last checked
    pub last_checked: String,
    /// last modified
    pub last_modified: String,
}

/// [`UrlType`]: useful for identifying amongst url feed source types, like rss vs fediverse
/// Unused atm.
#[derive(Debug, Default, Serialize, Deserialize)]
pub enum UrlType {
    #[default]
    Rss,
}

// TODO(swarnim): handle fediverse URLs
// we have to use https://docs.joinmastodon.org/methods/streaming/#public
// for checking up on public federated timeline
// the problems:
// we can't access the timeline anonymously https://github.com/mastodon/mastodon/pull/23989#issuecomment-1628961709
// figure out if there is an alternative or if we need user's api token key?
// or setup an embedded fediverse instance (let's keep this one as the last option)
// A better option would be to just use fedi buzz relay. (Will look into it, but putting more pressure on their service
// is not something I am sure I am comfortable with)
