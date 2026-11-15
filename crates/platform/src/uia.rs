use std::sync::Arc;

use otto_core::{AccessibleNode, AccessibleRole, AccessibleTree, Rect};
use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayCreateVector, SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::{VARIANT, VT_I4};
use windows::Win32::UI::Accessibility::{
    IInvokeProvider, IInvokeProvider_Impl, IRawElementProviderFragment,
    IRawElementProviderFragment_Impl, IRawElementProviderFragmentRoot,
    IRawElementProviderFragmentRoot_Impl, IRawElementProviderSimple,
    IRawElementProviderSimple_Impl, NavigateDirection, NavigateDirection_FirstChild,
    NavigateDirection_LastChild, NavigateDirection_NextSibling, NavigateDirection_Parent,
    NavigateDirection_PreviousSibling, ProviderOptions, ProviderOptions_ServerSideProvider,
    UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId, UIA_ControlTypePropertyId,
    UIA_InvokePatternId, UIA_IsContentElementPropertyId, UIA_IsControlElementPropertyId,
    UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId, UIA_NamePropertyId, UIA_PATTERN_ID,
    UIA_PROPERTY_ID, UIA_SliderControlTypeId, UIA_TabItemControlTypeId, UIA_ToolBarControlTypeId,
    UiaAppendRuntimeId, UiaHostProviderFromHwnd, UiaRect, UiaReturnRawElementProvider,
};
use windows::Win32::UI::WindowsAndMessaging::{OBJID_CLIENT, PostMessageW};
use windows_core::{BSTR, ComObject, ComObjectInterface, IUnknown, Interface, Result, implement};

pub const WM_APP_UIA_INVOKE: u32 = 0x8000 + 61;

#[allow(non_upper_case_globals, clippy::inline_always)]
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IRawElementProviderFragmentRoot
)]
struct SurfaceRootProvider {
    hwnd: HWND,
    invoke_message: u32,
    tree: Arc<AccessibleTree>,
}

#[allow(clippy::inline_always)]
#[implement(
    IRawElementProviderSimple,
    IRawElementProviderFragment,
    IInvokeProvider
)]
struct SurfaceChildProvider {
    hwnd: HWND,
    invoke_message: u32,
    tree: Arc<AccessibleTree>,
    index: usize,
}

impl SurfaceRootProvider {
    fn new(hwnd: HWND, invoke_message: u32, tree: Arc<AccessibleTree>) -> Self {
        Self {
            hwnd,
            invoke_message,
            tree,
        }
    }

    fn child_fragment(&self, index: usize) -> Option<IRawElementProviderFragment> {
        if index >= self.tree.nodes.len() {
            return None;
        }
        let child = ComObject::new(SurfaceChildProvider {
            hwnd: self.hwnd,
            invoke_message: self.invoke_message,
            tree: Arc::clone(&self.tree),
            index,
        });
        Some(child.to_interface())
    }
}

impl SurfaceChildProvider {
    fn node(&self) -> &AccessibleNode {
        &self.tree.nodes[self.index]
    }

    fn root_fragment(&self) -> IRawElementProviderFragment {
        ComObject::new(SurfaceRootProvider::new(
            self.hwnd,
            self.invoke_message,
            Arc::clone(&self.tree),
        ))
        .to_interface()
    }

    fn root_fragment_root(&self) -> IRawElementProviderFragmentRoot {
        ComObject::new(SurfaceRootProvider::new(
            self.hwnd,
            self.invoke_message,
            Arc::clone(&self.tree),
        ))
        .to_interface()
    }

    fn sibling(&self, index: usize) -> Result<IRawElementProviderFragment> {
        if index >= self.tree.nodes.len() {
            return Err(windows_core::Error::empty());
        }
        Ok(ComObject::new(Self {
            hwnd: self.hwnd,
            invoke_message: self.invoke_message,
            tree: Arc::clone(&self.tree),
            index,
        })
        .to_interface())
    }
}

fn rect_to_uia(rect: Rect) -> UiaRect {
    UiaRect {
        left: f64::from(rect.x),
        top: f64::from(rect.y),
        width: f64::from(rect.width.max(0)),
        height: f64::from(rect.height.max(0)),
    }
}

fn control_type_id(role: AccessibleRole) -> i32 {
    match role {
        AccessibleRole::Toolbar => UIA_ToolBarControlTypeId.0,
        AccessibleRole::Button => UIA_ButtonControlTypeId.0,
        AccessibleRole::Tab => UIA_TabItemControlTypeId.0,
        AccessibleRole::Slider => UIA_SliderControlTypeId.0,
    }
}

fn create_i4_safearray(values: &[i32]) -> Result<*mut SAFEARRAY> {
    let array = unsafe { SafeArrayCreateVector(VT_I4, 0, values.len() as u32) };
    if array.is_null() {
        return Err(E_INVALIDARG.into());
    }
    let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
    let access = unsafe { SafeArrayAccessData(array, &raw mut data) };
    if access.is_err() || data.is_null() {
        return Err(access.err().unwrap_or_else(|| E_INVALIDARG.into()));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(values.as_ptr(), data.cast::<i32>(), values.len());
        SafeArrayUnaccessData(array)?;
    }
    Ok(array)
}

#[allow(non_upper_case_globals)]
impl IRawElementProviderSimple_Impl for SurfaceRootProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, _patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        Err(windows_core::Error::empty())
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let value = match propertyid {
            UIA_ControlTypePropertyId => VARIANT::from(UIA_ToolBarControlTypeId.0),
            UIA_NamePropertyId => VARIANT::from(BSTR::from(self.tree.root_name.as_str())),
            UIA_AutomationIdPropertyId => {
                VARIANT::from(BSTR::from(self.tree.root_automation_id.as_str()))
            }
            UIA_IsEnabledPropertyId
            | UIA_IsControlElementPropertyId
            | UIA_IsContentElementPropertyId => VARIANT::from(true),
            UIA_IsKeyboardFocusablePropertyId => VARIANT::from(false),
            _ => VARIANT::default(),
        };
        Ok(value)
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        unsafe { UiaHostProviderFromHwnd(self.hwnd) }
    }
}

#[allow(non_upper_case_globals)]
impl IRawElementProviderFragment_Impl for SurfaceRootProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        let target = match direction {
            NavigateDirection_FirstChild => self.child_fragment(0),
            NavigateDirection_LastChild => self
                .tree
                .nodes
                .len()
                .checked_sub(1)
                .and_then(|idx| self.child_fragment(idx)),
            _ => None,
        };
        target.ok_or_else(windows_core::Error::empty)
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        Err(E_NOTIMPL.into())
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(rect_to_uia(self.tree.root_rect))
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Err(windows_core::Error::empty())
    }

    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(
            ComObjectInterface::<IRawElementProviderFragmentRoot>::as_interface_ref(self)
                .to_owned(),
        )
    }
}

impl IRawElementProviderFragmentRoot_Impl for SurfaceRootProvider_Impl {
    fn ElementProviderFromPoint(&self, x: f64, y: f64) -> Result<IRawElementProviderFragment> {
        let ix = x.round() as i32;
        let iy = y.round() as i32;
        for (idx, node) in self.tree.nodes.iter().enumerate() {
            if node.rect.contains(ix, iy)
                && let Some(fragment) = self.child_fragment(idx)
            {
                return Ok(fragment);
            }
        }
        Err(windows_core::Error::empty())
    }

    fn GetFocus(&self) -> Result<IRawElementProviderFragment> {
        Err(windows_core::Error::empty())
    }
}

#[allow(non_upper_case_globals)]
impl IRawElementProviderSimple_Impl for SurfaceChildProvider_Impl {
    fn ProviderOptions(&self) -> Result<ProviderOptions> {
        Ok(ProviderOptions_ServerSideProvider)
    }

    fn GetPatternProvider(&self, patternid: UIA_PATTERN_ID) -> Result<IUnknown> {
        if patternid == UIA_InvokePatternId && self.node().invoke_tag.is_some() {
            let invoke = ComObjectInterface::<IInvokeProvider>::as_interface_ref(self);
            return invoke.cast::<IUnknown>();
        }
        Err(windows_core::Error::empty())
    }

    fn GetPropertyValue(&self, propertyid: UIA_PROPERTY_ID) -> Result<VARIANT> {
        let node = self.node();
        let value = match propertyid {
            UIA_ControlTypePropertyId => VARIANT::from(control_type_id(node.role)),
            UIA_NamePropertyId => VARIANT::from(BSTR::from(node.name.as_str())),
            UIA_AutomationIdPropertyId => VARIANT::from(BSTR::from(node.automation_id.as_str())),
            UIA_IsEnabledPropertyId => VARIANT::from(node.enabled),
            UIA_IsControlElementPropertyId
            | UIA_IsContentElementPropertyId
            | UIA_IsKeyboardFocusablePropertyId => VARIANT::from(true),
            _ => VARIANT::default(),
        };
        Ok(value)
    }

    fn HostRawElementProvider(&self) -> Result<IRawElementProviderSimple> {
        Err(windows_core::Error::empty())
    }
}

#[allow(non_upper_case_globals)]
impl IRawElementProviderFragment_Impl for SurfaceChildProvider_Impl {
    fn Navigate(&self, direction: NavigateDirection) -> Result<IRawElementProviderFragment> {
        match direction {
            NavigateDirection_Parent => Ok(self.root_fragment()),
            NavigateDirection_NextSibling => self.sibling(self.index + 1),
            NavigateDirection_PreviousSibling => self.index.checked_sub(1).map_or_else(
                || Err(windows_core::Error::empty()),
                |idx| self.sibling(idx),
            ),
            _ => Err(windows_core::Error::empty()),
        }
    }

    fn GetRuntimeId(&self) -> Result<*mut SAFEARRAY> {
        let node = self.node();
        create_i4_safearray(&[UiaAppendRuntimeId as i32, node.runtime_id])
    }

    fn BoundingRectangle(&self) -> Result<UiaRect> {
        Ok(rect_to_uia(self.node().rect))
    }

    fn GetEmbeddedFragmentRoots(&self) -> Result<*mut SAFEARRAY> {
        Err(windows_core::Error::empty())
    }

    fn SetFocus(&self) -> Result<()> {
        Ok(())
    }

    fn FragmentRoot(&self) -> Result<IRawElementProviderFragmentRoot> {
        Ok(self.root_fragment_root())
    }
}

impl IInvokeProvider_Impl for SurfaceChildProvider_Impl {
    fn Invoke(&self) -> Result<()> {
        if let Some(tag) = self.node().invoke_tag {
            unsafe {
                let _ = PostMessageW(
                    Some(self.hwnd),
                    self.invoke_message,
                    WPARAM(tag as usize),
                    LPARAM(0),
                );
            }
        }
        Ok(())
    }
}

pub fn handle_wm_getobject(
    hwnd: isize,
    wparam: usize,
    lparam: isize,
    invoke_message: u32,
    build_tree: impl FnOnce() -> AccessibleTree,
) -> Option<isize> {
    if hwnd == 0 || (lparam as i32) != OBJID_CLIENT.0 {
        return None;
    }
    let hwnd = HWND(hwnd as *mut _);
    let tree = Arc::new(build_tree());
    let root = ComObject::new(SurfaceRootProvider::new(hwnd, invoke_message, tree));
    let provider: IRawElementProviderSimple = root.to_interface();
    let result = unsafe {
        UiaReturnRawElementProvider(hwnd, WPARAM(wparam), LPARAM(lparam), Some(&provider))
    };
    Some(result.0)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use otto_core::{AccessibleNode, AccessibleRole, AccessibleTree, Rect};
    use windows::Win32::System::Ole::SafeArrayDestroy;
    use windows::Win32::UI::Accessibility::{
        IInvokeProvider, IRawElementProviderFragment, IRawElementProviderFragmentRoot,
        IRawElementProviderSimple, NavigateDirection_FirstChild, NavigateDirection_NextSibling,
        NavigateDirection_Parent, UIA_AutomationIdPropertyId, UIA_ButtonControlTypeId,
        UIA_ControlTypePropertyId, UIA_InvokePatternId, UIA_NamePropertyId,
    };
    use windows_core::{ComObject, Interface};

    use super::{SurfaceRootProvider, WM_APP_UIA_INVOKE, handle_wm_getobject};

    fn sample_tree() -> AccessibleTree {
        AccessibleTree {
            root_name: String::from("Otto Top Bar"),
            root_automation_id: String::from("otto.bar"),
            root_rect: Rect {
                x: 0,
                y: 0,
                width: 1920,
                height: 36,
            },
            nodes: vec![
                AccessibleNode {
                    runtime_id: 1,
                    automation_id: String::from("otto.bar.start"),
                    name: String::from("Otto Menu"),
                    role: AccessibleRole::Button,
                    rect: Rect {
                        x: 8,
                        y: 4,
                        width: 28,
                        height: 28,
                    },
                    enabled: true,
                    invoke_tag: Some(1),
                },
                AccessibleNode {
                    runtime_id: 9,
                    automation_id: String::from("otto.notch"),
                    name: String::from("Notch"),
                    role: AccessibleRole::Button,
                    rect: Rect {
                        x: 860,
                        y: 0,
                        width: 200,
                        height: 34,
                    },
                    enabled: true,
                    invoke_tag: Some(9),
                },
            ],
        }
    }

    #[test]
    fn root_and_child_providers_navigate_hit_test_and_expose_invoke_pattern() {
        let root_obj = ComObject::new(SurfaceRootProvider::new(
            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
            WM_APP_UIA_INVOKE,
            Arc::new(sample_tree()),
        ));
        let root_simple: IRawElementProviderSimple = root_obj.to_interface();
        let name = unsafe { root_simple.GetPropertyValue(UIA_NamePropertyId) }.unwrap();
        assert_eq!(name.to_string(), "Otto Top Bar");

        let root_frag: IRawElementProviderFragment = root_simple.cast().unwrap();
        let first = unsafe { root_frag.Navigate(NavigateDirection_FirstChild) }.unwrap();
        let second = unsafe { first.Navigate(NavigateDirection_NextSibling) }.unwrap();
        let parent = unsafe { second.Navigate(NavigateDirection_Parent) }.unwrap();
        let parent_simple: IRawElementProviderSimple = parent.cast().unwrap();
        assert_eq!(
            unsafe { parent_simple.GetPropertyValue(UIA_AutomationIdPropertyId) }
                .unwrap()
                .to_string(),
            "otto.bar"
        );

        let second_simple: IRawElementProviderSimple = second.cast().unwrap();
        assert_eq!(
            i32::try_from(
                &unsafe { second_simple.GetPropertyValue(UIA_ControlTypePropertyId) }.unwrap()
            )
            .unwrap(),
            UIA_ButtonControlTypeId.0
        );
        let runtime_id = unsafe { second.GetRuntimeId() }.unwrap();
        assert!(!runtime_id.is_null());
        unsafe {
            let _ = SafeArrayDestroy(runtime_id);
        }

        let pattern = unsafe { second_simple.GetPatternProvider(UIA_InvokePatternId) }.unwrap();
        let invoke: IInvokeProvider = pattern.cast().unwrap();
        unsafe { invoke.Invoke() }.unwrap();

        let root_hit: IRawElementProviderFragmentRoot = root_simple.cast().unwrap();
        let hit = unsafe { root_hit.ElementProviderFromPoint(900.0, 12.0) }.unwrap();
        let hit_simple: IRawElementProviderSimple = hit.cast().unwrap();
        assert_eq!(
            unsafe { hit_simple.GetPropertyValue(UIA_AutomationIdPropertyId) }
                .unwrap()
                .to_string(),
            "otto.notch"
        );
    }

    #[test]
    fn non_client_getobject_is_ignored_without_building_tree() {
        let mut built = false;
        let res = handle_wm_getobject(123, 0, 1, WM_APP_UIA_INVOKE, || {
            built = true;
            sample_tree()
        });
        assert_eq!(res, None);
        assert!(!built);
    }
}
