/// Link health as reported by a data source's frame (GnssFrame, Bno085Frame): whether it's
/// live with recent real data, showing no recent data at all, or knowingly fed by a synthetic
/// test source rather than real hardware. `Test` takes priority over staleness -- a synthetic
/// frame that happens to go stale (e.g. its generator thread stalls) should still read as
/// "this is test data", not "link down".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Ok,
    NoData,
    Test,
}
