use std::collections::HashSet;

use crate::domain::action::InteractionContext;
use windows::{
    core::{w, Interface, GUID, PCWSTR},
    Win32::{
        Foundation::HWND,
        System::{
            Com::{
                CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
                DISPATCH_PROPERTYGET, DISPPARAMS,
            },
            Ole::GetActiveObject,
            Variant::VARIANT,
        },
        UI::Accessibility::{
            CUIAutomation, IUIAutomation, UIA_ComboBoxControlTypeId, UIA_DocumentControlTypeId,
            UIA_EditControlTypeId, UIA_ImageControlTypeId, UIA_ListControlTypeId,
            UIA_ListItemControlTypeId, UIA_SelectionItemPatternId, UIA_SelectionPatternId,
            UIA_TextControlTypeId, UIA_TextEditPatternId, UIA_TextPatternId, UIA_ValuePatternId,
        },
    },
};

use super::context::get_app_process_name;

const POWERPOINT_PROCESS: &str = "POWERPNT.EXE";

const PP_SELECTION_SLIDES: i32 = 1;
const PP_SELECTION_SHAPES: i32 = 2;
const PP_SELECTION_TEXT: i32 = 3;

const MSO_LINKED_PICTURE: i32 = 11;
const MSO_PICTURE: i32 = 13;

pub fn detect_active_interaction(hwnd: HWND) -> InteractionContext {
    let mut tags = HashSet::new();

    if let Err(err) = detect_uia_tags(&mut tags) {
        log::debug!("UI Automation context detection failed: {err}");
    }

    if is_powerpoint_window(hwnd) {
        tags.insert("app.powerpoint".to_string());
        if let Err(err) = detect_powerpoint_tags(&mut tags) {
            log::debug!("PowerPoint context detection failed: {err}");
        }
    }

    InteractionContext::from_tags(tags)
}

fn is_powerpoint_window(hwnd: HWND) -> bool {
    get_app_process_name(&hwnd).is_some_and(|name| name.eq_ignore_ascii_case(POWERPOINT_PROCESS))
}

fn detect_uia_tags(tags: &mut HashSet<String>) -> windows::core::Result<()> {
    let _com = ComApartment::initialize();
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }?;
    let element = unsafe { automation.GetFocusedElement() }?;

    if let Ok(control_type) = unsafe { element.CurrentControlType() } {
        if control_type == UIA_EditControlTypeId {
            tags.insert("ui.text_input".to_string());
            tags.insert("ui.edit".to_string());
        } else if control_type == UIA_DocumentControlTypeId {
            tags.insert("ui.document".to_string());
        } else if control_type == UIA_TextControlTypeId {
            tags.insert("ui.text".to_string());
        } else if control_type == UIA_ImageControlTypeId {
            tags.insert("ui.image".to_string());
        } else if control_type == UIA_ListControlTypeId || control_type == UIA_ListItemControlTypeId
        {
            tags.insert("ui.list".to_string());
        } else if control_type == UIA_ComboBoxControlTypeId {
            tags.insert("ui.combo_box".to_string());
        }
    }

    if unsafe { element.GetCurrentPattern(UIA_ValuePatternId) }.is_ok() {
        tags.insert("ui.value".to_string());
    }
    if unsafe { element.GetCurrentPattern(UIA_TextPatternId) }.is_ok()
        || unsafe { element.GetCurrentPattern(UIA_TextEditPatternId) }.is_ok()
    {
        tags.insert("ui.text_input".to_string());
    }
    if unsafe { element.GetCurrentPattern(UIA_SelectionPatternId) }.is_ok()
        || unsafe { element.GetCurrentPattern(UIA_SelectionItemPatternId) }.is_ok()
    {
        tags.insert("ui.selection".to_string());
    }

    Ok(())
}

fn detect_powerpoint_tags(tags: &mut HashSet<String>) -> windows::core::Result<()> {
    let _com = ComApartment::initialize();
    let clsid = unsafe { CLSIDFromProgID(w!("PowerPoint.Application")) }?;
    let mut unknown = None;
    unsafe { GetActiveObject(&clsid, None, &mut unknown) }?;
    let app: IDispatch = unknown.ok_or_else(windows::core::Error::empty)?.cast()?;

    let active_window = dispatch_property_dispatch(&app, "ActiveWindow")?;
    let selection = dispatch_property_dispatch(&active_window, "Selection")?;
    let selection_type = dispatch_property_i32(&selection, "Type")?;

    match selection_type {
        PP_SELECTION_SLIDES => {
            tags.insert("ppt.selection.slide".to_string());
        }
        PP_SELECTION_SHAPES => {
            tags.insert("ppt.selection.shape".to_string());
            if selected_powerpoint_shape_is_picture(&selection).unwrap_or(false) {
                tags.insert("ppt.selection.picture".to_string());
            }
        }
        PP_SELECTION_TEXT => {
            tags.insert("ppt.selection.text".to_string());
            tags.insert("ui.text_input".to_string());
        }
        _ => {}
    }

    Ok(())
}

fn selected_powerpoint_shape_is_picture(selection: &IDispatch) -> windows::core::Result<bool> {
    let shape_range = dispatch_property_dispatch(selection, "ShapeRange")?;
    let shape_type = dispatch_property_i32(&shape_range, "Type")?;
    Ok(matches!(shape_type, MSO_LINKED_PICTURE | MSO_PICTURE))
}

fn dispatch_property_dispatch(
    dispatch: &IDispatch,
    property: &str,
) -> windows::core::Result<IDispatch> {
    let variant = dispatch_property(dispatch, property)?;
    IDispatch::try_from(&variant)
}

fn dispatch_property_i32(dispatch: &IDispatch, property: &str) -> windows::core::Result<i32> {
    let variant = dispatch_property(dispatch, property)?;
    i32::try_from(&variant)
}

fn dispatch_property(dispatch: &IDispatch, property: &str) -> windows::core::Result<VARIANT> {
    let dispid = dispatch_id(dispatch, property)?;
    let params = DISPPARAMS::default();
    let mut result = VARIANT::default();
    unsafe {
        dispatch.Invoke(
            dispid,
            &GUID::zeroed(),
            0,
            DISPATCH_PROPERTYGET,
            &params,
            Some(&mut result),
            None,
            None,
        )?;
    }
    Ok(result)
}

fn dispatch_id(dispatch: &IDispatch, name: &str) -> windows::core::Result<i32> {
    let mut wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let name_ptr = PCWSTR(wide_name.as_mut_ptr());
    let mut dispid = 0;
    unsafe {
        dispatch.GetIDsOfNames(&GUID::zeroed(), &name_ptr, 1, 0, &mut dispid)?;
    }
    Ok(dispid)
}

struct ComApartment {
    should_uninitialize: bool,
}

impl ComApartment {
    fn initialize() -> Self {
        let flags = COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE;
        let hr = unsafe { CoInitializeEx(None, flags) };
        Self {
            should_uninitialize: hr.is_ok(),
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}
