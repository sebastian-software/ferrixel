#![no_main]

// The harness runs *outside* the safe core and requires sandbox/process
// memory limits. AVIF/JXL codecs may allocate opaque full-frame buffers
// before Ferrixel's engine-managed memory budget can be enforced.
use ferrixel::pipeline::{
    ImageFormat, InputProfilePolicy, TransformOptions, transcode,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() < 2 || bytes.len() > 512 * 1024 {
        return;
    }
    let kind = match bytes[0] % 3 {
        0 => ImageFormat::Jpeg,
        1 => ImageFormat::JpegXl,
        _ => ImageFormat::Avif,
    };
    let options = TransformOptions {
        output_width: 1 + u32::from(bytes[0] % 8),
        output_height: 1 + u32::from(bytes[1] % 8),
        quality: 60.0,
        speed: 10,
        max_managed_bytes: 2 * 1024 * 1024,
        profile_policy: InputProfilePolicy::Strict,
        ..TransformOptions::default()
    };
    let _ = transcode(&bytes[1..], kind, options);
});
