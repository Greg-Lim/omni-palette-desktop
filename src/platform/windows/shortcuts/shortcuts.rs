// Tooling to automatically detect available shortcuts on current window. Does not work well and probably should be scrape

use windows::core::HSTRING;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow; // Needed for the test

// Define a struct to hold the results
pub struct AcceleratorInfo {
    pub name: String,
    pub key: String,
}

pub fn get_all_accelerators(hwnd: HWND) -> Result<Vec<AcceleratorInfo>, windows::core::Error> {
    // 1. Initialize COM
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    // 2. Create the main IUIAutomation object (the entry point)
    let automation: IUIAutomation =
        unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)? };

    // 3. Get the root element for the specified HWND
    let root_element = unsafe { automation.ElementFromHandle(hwnd)? };

    let mut results = Vec::new();

    // 4. Start the recursive search
    find_accelerators_recursive(&automation, &root_element, &mut results)?;

    Ok(results)
}

// Helper function for traversing the tree and checking properties
fn find_accelerators_recursive(
    automation: &IUIAutomation,
    element: &IUIAutomationElement,
    results: &mut Vec<AcceleratorInfo>,
) -> Result<(), windows::core::Error> {
    let name = unsafe { element.CurrentName()? }.to_string();
    let keys = extract_shortcut_strings(element)?;

    for key in keys {
        results.push(AcceleratorInfo {
            name: name.clone(),
            key,
        });
    }

    let ct = unsafe { element.CurrentControlType()? };
    let cls = unsafe { element.CurrentClassName()? }.to_string();
    let aid = unsafe { element.CurrentAutomationId()? }.to_string();
    println!("ct={:?} class={} aid={} name={}", ct, cls, aid, unsafe {
        element.CurrentName()?
    });

    let true_condition = unsafe { automation.CreateTrueCondition()? };
    let children = unsafe { element.FindAll(TreeScope_Children, &true_condition)? };

    let count = unsafe { children.Length()? };
    for i in 0..count {
        let child = unsafe { children.GetElement(i)? };
        find_accelerators_recursive(automation, &child, results)?;
    }

    Ok(())
}

fn extract_shortcut_strings(
    element: &IUIAutomationElement,
) -> Result<Vec<String>, windows::core::Error> {
    let mut out = Vec::new();

    let name = unsafe { element.CurrentName()? }.to_string();
    let accel = unsafe { element.CurrentAcceleratorKey()? }.to_string();
    let access = unsafe { element.CurrentAccessKey()? }.to_string();

    if !accel.is_empty() {
        out.push(accel);
    }
    if !access.is_empty() {
        out.push(access);
    }

    // Many apps (Chrome included) store shortcuts like: "Copy\tCtrl+C"
    if let Some((_, rhs)) = name.split_once('\t') {
        let s = rhs.trim().to_string();
        if !s.is_empty() {
            out.push(s);
        }
    }

    // Legacy IAccessible keyboard shortcut
    if let Ok(legacy) = unsafe {
        element.GetCurrentPatternAs::<IUIAutomationLegacyIAccessiblePattern>(
            UIA_LegacyIAccessiblePatternId,
        )
    } {
        let s = unsafe { legacy.CurrentKeyboardShortcut()? }.to_string();
        if !s.is_empty() {
            out.push(s);
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // A note on testing UI code: This test is an *integration test* because it
    // relies on the state of the Windows desktop and runs potentially slow, external code.
    // It should be run with 'cargo test -- --nocapture' to see the println! output.

    #[test]
    fn test_get_all_accelerators_and_print() {
        // Get the HWND of the window currently in the foreground when the test runs
        let hwnd = unsafe { GetForegroundWindow() };

        if hwnd.is_invalid() {
            println!("No foreground window found. Skipping accelerator check.");
            return;
        }

        println!(
            "\n--- Accelerator Keys for Foreground Window (HWND: {:?}) ---",
            hwnd
        );

        match get_all_accelerators(hwnd) {
            Ok(accelerators) => {
                if accelerators.is_empty() {
                    println!("No accelerator keys found in this window.");
                } else {
                    // Print the header
                    println!("{: <40} | {}", "Control Name", "Accelerator Key");
                    println!("{}", "-".repeat(60));

                    // Print all found accelerator information
                    for info in accelerators {
                        println!("{: <40} | {}", info.name, info.key);
                    }
                }
            }
            Err(e) => {
                // This often happens if the COM initialisation or UIA calls fail
                eprintln!("Error retrieving accelerators: {}", e);
                // Fail the test if a critical error occurs
                panic!("UI Automation failed: {}", e);
            }
        }
    }
}
