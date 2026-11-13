//! Watches the system's default audio output device (macOS).
//!
//! CoreAudio fires the callback whenever the default output changes — the user
//! switching to AirPlay or headphones mid-track, or devices re-appearing after
//! sleep. The callback runs on a CoreAudio/run-loop thread, so it must stay
//! tiny: the player registers one that just posts a message to its own thread.

use coreaudio_sys::{
    kAudioHardwarePropertyDefaultOutputDevice, kAudioObjectPropertyElementMaster,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, AudioObjectAddPropertyListener,
    AudioObjectID, AudioObjectPropertyAddress, OSStatus,
};

/// Double indirection so the fat closure pointer fits through a single c_void
/// (the same shape cpal uses for its own property listeners)
struct Callback(Box<dyn FnMut() + Send>);

unsafe extern "C" fn shim(
    _id: AudioObjectID,
    _n_addresses: u32,
    _addresses: *const AudioObjectPropertyAddress,
    data: *mut std::os::raw::c_void,
) -> OSStatus {
    let callback = data as *mut Callback;
    (*callback).0();
    0
}

/// Register a process-lifetime listener for default-output-device changes.
/// Returns false when CoreAudio refuses the registration; the callback box is
/// intentionally leaked because the listener is never removed.
pub fn watch_default_output(on_change: impl FnMut() + Send + 'static) -> bool {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDefaultOutputDevice,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    let callback = Box::into_raw(Box::new(Callback(Box::new(on_change))));
    let status = unsafe {
        AudioObjectAddPropertyListener(
            kAudioObjectSystemObject,
            &address,
            Some(shim),
            callback as *mut _,
        )
    };
    status == 0
}
