use crate::rdev::{Event, EventType, GrabError};
use crate::windows::common::{
    convert, get_scan_code, set_key_hook, set_mouse_hook, HookError, HOOK, KEYBOARD,
};
use std::ptr::null_mut;
use std::time::SystemTime;
use winapi::um::winuser::{CallNextHookEx, GetMessageA, HC_ACTION};

static mut GLOBAL_CALLBACK: Option<Box<dyn FnMut(Event) -> Option<Event>>> = None;
static mut GET_KEY_UNICODE: bool = true;

pub fn set_get_key_unicode(b: bool) {
    unsafe {
        GET_KEY_UNICODE = b;
    }
}

pub fn set_event_popup(b: bool) {
    KEYBOARD.lock().unwrap().set_event_popup(b);
}

unsafe extern "system" fn raw_callback(code: i32, param: usize, lpdata: isize) -> isize {
    if code == HC_ACTION {
        let (opt, code) = convert(param, lpdata);
        if let Some(event_type) = opt {
            let unicode = if GET_KEY_UNICODE {
                match &event_type {
                    EventType::KeyPress(key) => {
                        if key.is_alpha() {
                            match (*KEYBOARD).lock() {
                                Ok(mut keyboard) => keyboard.get_unicode(lpdata),
                                Err(_) => None,
                            }
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            let event = Event {
                event_type,
                time: SystemTime::now(),
                unicode,
                code,
                scan_code: get_scan_code(lpdata),
            };
            if let Some(callback) = &mut GLOBAL_CALLBACK {
                if callback(event).is_none() {
                    // https://stackoverflow.com/questions/42756284/blocking-windows-mouse-click-using-setwindowshookex
                    // https://android.developreference.com/article/14560004/Blocking+windows+mouse+click+using+SetWindowsHookEx()
                    // https://cboard.cprogramming.com/windows-programming/99678-setwindowshookex-wm_keyboard_ll.html
                    // let _result = CallNextHookEx(HOOK, code, param, lpdata);
                    return 1;
                }
            }
        }
    }
    CallNextHookEx(HOOK, code, param, lpdata)
}
impl From<HookError> for GrabError {
    fn from(error: HookError) -> Self {
        match error {
            HookError::Mouse(code) => GrabError::MouseHookError(code),
            HookError::Key(code) => GrabError::KeyHookError(code),
        }
    }
}

pub fn grab<T>(callback: T) -> Result<(), GrabError>
where
    T: FnMut(Event) -> Option<Event> + 'static,
{
    unsafe {
        GLOBAL_CALLBACK = Some(Box::new(callback));
        set_key_hook(raw_callback)?;
        if !crate::keyboard_only() {
            set_mouse_hook(raw_callback)?;
        }

        GetMessageA(null_mut(), null_mut(), 0, 0);
    }
    Ok(())
}
