/// Data as returned by Ariel OS's `/id` endpoint.
///
/// See <https://github.com/ariel-os/ariel-os/pull/2417>, or (when merged)
/// <https://ariel-os.github.io/ariel-os/dev/docs/book/tooling/coap.html>.
#[derive(minicbor::Decode, Default)]
#[cbor(map, tag(3050242967))]
pub struct ImplInfo {
    #[n(1)]
    pub os_name: Option<String>,
    #[n(2)]
    pub os_version: Option<String>,
    #[n(6)]
    pub board: Option<String>,
    #[n(8)]
    pub device_id: Option<minicbor::bytes::ByteVec>,
}
