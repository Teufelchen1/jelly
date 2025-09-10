//! Crawler that accesses all resources, and follows links from any link-format resource.

use super::asynch::prelude::*;

use coap_lite::link_format::LinkFormatParser;
use coap_lite::CoapRequest;
use coap_lite::RequestType as Method;

use eyre::eyre;

pub struct Spider;

impl super::asynch::AsyncCommand for Spider {
    const COMMAND: &str = "spider";
    const DESCRIPTION: &str = "GETs all resources on the web server";

    #[allow(
        refining_impl_trait,
        reason = "Making error type inference work is odd in any other way"
    )]
    async fn run(args: &str, mut main: MainProgram) -> eyre::Result<()> {
        if args != "spider" {
            return Err(eyre::Error::msg("spider takes no arguments"));
        }

        let mut queue = OnceQueue::default();
        queue.push(".well-known/core");

        while let Some(current) = queue.pop() {
            // Generally it may be a good idea to sleep, but as the main loop's interface always
            // needs a next CoAP request, sleeping would block the UI.
            //
            // smol::Timer::after(std::time::Duration::from_secs(1)).await;

            writeln!(
                main,
                "[spider] GETting {current:?}, {} remaining.",
                queue.pending.len()
            )
            .unwrap();

            let mut request = CoapRequest::new();
            request.set_method(Method::Get);
            request.set_path(&current);

            let response = main.request(request).await;

            let coap_lite::MessageClass::Response(code) = response.header.code else {
                // We won't complain about every single thing that'd make us not process a
                // response. Even though this one (empty or request code in response) would be
                // *really* weird.
                continue;
            };
            if code.is_error() {
                continue;
            }
            if response.get_content_format()
                == Some(coap_lite::ContentFormat::ApplicationLinkFormat)
            {
                let Ok(payload) = str::from_utf8(&response.payload) else {
                    continue;
                };
                let mut actually_new = 0;
                let mut discovered = 0;
                for link in LinkFormatParser::new(payload) {
                    let Ok(href) = find_local_target(link) else {
                        continue;
                    };
                    actually_new += queue.push(&href) as usize;
                    discovered += 1;
                }
                writeln!(
                    main,
                    "[spider] Found {discovered} links, thereof {actually_new} new."
                )?;
            }
        }

        Ok(())
    }
}

/// Utility queue structure that ignores duplicate insertion even after the first value has been
/// popped.
#[derive(Default, Debug)]
struct OnceQueue {
    seen: std::collections::HashSet<String>,
    pending: std::collections::VecDeque<String>,
}

impl OnceQueue {
    /// Insert a value into the queue, returning `true` if it is a new one.
    fn push(&mut self, value: &str) -> bool {
        if self.seen.contains(value) {
            return false;
        }
        self.seen.insert(value.to_string());
        self.pending.push_back(value.to_string());
        true
    }

    /// Pop a value that has been inserted at least once and never popped before.
    fn pop(&mut self) -> Option<String> {
        self.pending.pop_back()
    }
}

/// Finds the path and query (`/foo/bar?baz=qux`) of an RFC6690 link if it is a local link.
fn find_local_target(link: <LinkFormatParser as Iterator>::Item) -> eyre::Result<String> {
    // Note that it matters that we run on `https://`, because WHATWG's origin rules work
    // differently for HTTP and "unknown" protocols.
    //
    // We're using a base URI because the `url` crate won't parse URI references on their own.
    let base_uri = url::Url::parse("https://slipmux/").expect("URI is known valid");
    let base_uri_for_comparison = base_uri.clone();

    let mut link = link.map_err(|e| eyre!("Parse error: {e:?}"))?;

    // RFC6690 hrefs are URI references and not WHATWG URLs, but the differences
    // are minor compared to RFC6690's oddities.
    //
    // In particular, any (even deeply starting) URI refrence always takes the
    // origin rather than the current address as a starting point. (Unless there is
    // an off-site anchor).
    let href_base = match link.1.find(|(key, _)| key == &"anchor") {
        Some((_, value)) => base_uri.join(&value.to_cow())?.join("/")?,
        None => base_uri,
    };
    let href = href_base.join(link.0)?;
    if href.origin() != base_uri_for_comparison.origin() {
        return Err(eyre!(
            "Not a host-local link ({:?} vs. {:?})",
            href.origin(),
            base_uri_for_comparison.origin()
        ));
    }

    let href = base_uri_for_comparison
        .make_relative(&href)
        .expect("Possible because they have the same origin");

    Ok(href)
}

#[test]
fn test_find_local_target() {
    let mut parsed = LinkFormatParser::new("<a>,<b>;anchor=\"/c\",<remote>;anchor=\"remote:///\"");
    assert_eq!(find_local_target(parsed.next().unwrap()).unwrap(), "a");
    assert_eq!(find_local_target(parsed.next().unwrap()).unwrap(), "b");
    assert!(find_local_target(parsed.next().unwrap()).is_err());
}
