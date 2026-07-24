#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::{Cell, OnceCell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSBorderType, NSColor, NSControl, NSControlTextEditingDelegate, NSFloatingWindowLevel,
    NSFocusRingType, NSFont, NSFontAttributeName, NSFontManager, NSFontTraitMask,
    NSFontWeightRegular, NSForegroundColorAttributeName, NSMenu, NSMenuItem, NSPanel, NSScrollView,
    NSTableColumn, NSTableView, NSTableViewDataSource, NSTableViewDelegate,
    NSTableViewRowSizeStyle, NSTableViewSelectionHighlightStyle, NSTableViewStyle, NSTextField,
    NSTextFieldDelegate, NSTextView, NSView, NSWindowCollectionBehavior, NSWindowDelegate,
    NSWindowStyleMask,
};
use objc2_foundation::{
    MainThreadMarker, NSEdgeInsets, NSIndexSet, NSMutableAttributedString, NSNotification,
    NSObject, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString,
};

use crate::{Color, Match, Options, filter_entries};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Output {
    Selection(usize),
    Input(String),
}

struct Model {
    options: Options,
    entries: Vec<String>,
    matches: Vec<Match>,
    selected: Option<usize>,
    result: Option<Output>,
}

impl Model {
    fn new(options: Options, entries: &[String]) -> Self {
        let matches = filter_entries(entries, &options.search, options.sort);
        let selected = (!matches.is_empty()).then_some(0);
        Self {
            options,
            entries: entries.to_vec(),
            matches,
            selected,
            result: None,
        }
    }
}

struct Theme {
    font: Retained<NSFont>,
    background: Retained<NSColor>,
    text: Retained<NSColor>,
    prompt: Retained<NSColor>,
    prompt_background: Retained<NSColor>,
    selection: Retained<NSColor>,
    selection_text: Retained<NSColor>,
    matched: Retained<NSColor>,
}

impl Theme {
    fn new(options: &Options, mtm: MainThreadMarker) -> Result<Self, String> {
        let font = match &options.font {
            Some(spec) => {
                let name = NSString::from_str(&spec.family);
                NSFont::fontWithName_size(&name, spec.size)
                    .or_else(|| {
                        NSFontManager::sharedFontManager(mtm).fontWithFamily_traits_weight_size(
                            &name,
                            NSFontTraitMask::empty(),
                            5,
                            spec.size,
                        )
                    })
                    .ok_or_else(|| format!("font {:?} is not installed", spec.family))?
            }
            // SAFETY: Reading this immutable AppKit framework constant is safe.
            None => NSFont::monospacedSystemFontOfSize_weight(16.0, unsafe { NSFontWeightRegular }),
        };

        Ok(Self {
            font,
            background: color(options.background, default_background),
            text: color(options.text_color, default_text),
            prompt: color(options.prompt_color, default_prompt_text),
            prompt_background: color(options.prompt_background, default_prompt_background),
            selection: color(options.selection_color, default_selection),
            selection_text: color(options.selection_text_color, default_selection_text),
            matched: color(options.match_color, default_match),
        })
    }
}

fn color(value: Option<Color>, fallback: fn() -> Retained<NSColor>) -> Retained<NSColor> {
    match value {
        Some(value) => NSColor::colorWithSRGBRed_green_blue_alpha(
            value.red,
            value.green,
            value.blue,
            value.alpha,
        ),
        None => fallback(),
    }
}

fn default_background() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.105, 0.09, 0.96)
}

fn default_text() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.84, 0.82, 0.79, 1.0)
}

fn default_prompt_text() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.105, 0.09, 1.0)
}

fn default_prompt_background() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.64, 0.61, 0.58, 1.0)
}

fn default_selection() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.74, 0.71, 0.68, 1.0)
}

fn default_selection_text() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.12, 0.105, 0.09, 1.0)
}

fn default_match() -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(0.96, 0.74, 0.42, 1.0)
}

fn list_heights(item_count: usize, visible_limit: usize, row_height: f64) -> (f64, f64) {
    (
        row_height * item_count.min(visible_limit) as f64,
        row_height * item_count as f64,
    )
}

define_class!(
    // SAFETY: NSPanel supports subclassing and PickerPanel does not implement Drop.
    #[unsafe(super = NSPanel)]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug)]
    struct PickerPanel;

    unsafe impl NSObjectProtocol for PickerPanel {}

    impl PickerPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

impl PickerPanel {
    fn new(frame: NSRect, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm);
        // SAFETY: This is NSPanel's designated initializer with a valid frame and style.
        unsafe {
            msg_send![this,
                initWithContentRect: frame,
                styleMask: NSWindowStyleMask::Borderless,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        }
    }
}

struct AppIvars {
    model: RefCell<Model>,
    theme: Theme,
    panel: OnceCell<Retained<PickerPanel>>,
    field: OnceCell<Retained<NSTextField>>,
    table: OnceCell<Retained<NSTableView>>,
    refreshing_table: Cell<bool>,
    finished: Cell<bool>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and Delegate does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIvars]
    struct Delegate;

    // SAFETY: These AppKit delegate protocols have no additional safety requirements.
    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn application_did_finish_launching(&self, _notification: &NSNotification) {
            self.build_ui();
        }
    }

    unsafe impl NSWindowDelegate for Delegate {
        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _notification: &NSNotification) {
            self.finish(None);
        }
    }

    unsafe impl NSControlTextEditingDelegate for Delegate {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            self.update_query();
        }

        #[unsafe(method(control:textView:doCommandBySelector:))]
        unsafe fn control_text_view_do_command(
            &self,
            _control: &NSControl,
            _text_view: &NSTextView,
            command: Sel,
        ) -> bool {
            if command == sel!(moveUp:) {
                self.move_selection(-1);
            } else if command == sel!(moveDown:) {
                self.move_selection(1);
            } else if command == sel!(pageUp:) {
                self.move_selection(-(self.ivars().model.borrow().options.lines as isize));
            } else if command == sel!(pageDown:) {
                self.move_selection(self.ivars().model.borrow().options.lines as isize);
            } else if command == sel!(insertNewline:) {
                self.accept();
            } else if command == sel!(cancelOperation:) {
                self.finish(None);
            } else {
                return false.into();
            }
            true
        }
    }

    unsafe impl NSTextFieldDelegate for Delegate {}

    unsafe impl NSTableViewDataSource for Delegate {
        #[unsafe(method(numberOfRowsInTableView:))]
        fn number_of_rows(&self, _table: &NSTableView) -> isize {
            self.ivars().model.borrow().matches.len() as isize
        }
    }

    unsafe impl NSTableViewDelegate for Delegate {
        #[unsafe(method_id(tableView:viewForTableColumn:row:))]
        fn view_for_row(
            &self,
            _table: &NSTableView,
            _column: Option<&NSTableColumn>,
            row: isize,
        ) -> Option<Retained<NSView>> {
            self.row_view(row as usize)
        }

        #[unsafe(method(tableViewSelectionDidChange:))]
        fn table_selection_did_change(&self, _notification: &NSNotification) {
            if self.ivars().refreshing_table.get() {
                return;
            }
            let table = self.ivars().table.get().expect("table must be initialized");
            let row = table.selectedRow();
            let selected = (row >= 0).then_some(row as usize);
            let changed = {
                let mut model = self.ivars().model.borrow_mut();
                let changed = model.selected != selected;
                model.selected = selected;
                changed
            };
            if changed {
                table.reloadData();
            }
        }
    }

    impl Delegate {
        #[unsafe(method(accept:))]
        fn accept_action(&self, _sender: &AnyObject) {
            self.accept();
        }
    }
);

impl Delegate {
    fn new(model: Model, theme: Theme, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppIvars {
            model: RefCell::new(model),
            theme,
            panel: OnceCell::new(),
            field: OnceCell::new(),
            table: OnceCell::new(),
            refreshing_table: Cell::new(false),
            finished: Cell::new(false),
        });
        // SAFETY: NSObject's init signature is correct for this subclass.
        unsafe { msg_send![super(this), init] }
    }

    fn build_ui(&self) {
        let mtm = self.mtm();
        let model = self.ivars().model.borrow();
        let font_size = model.options.font.as_ref().map_or(16.0, |font| font.size);
        let row_height = (font_size * 1.45).ceil().max(22.0);
        let query_height = row_height;
        let (list_height, document_height) =
            list_heights(model.matches.len(), model.options.lines, row_height);
        let width = model.options.width;
        let height = query_height + list_height;
        let prompt = model.options.prompt.clone();
        let search = model.options.search.clone();
        drop(model);

        let prompt_field = NSTextField::labelWithString(&NSString::from_str(&prompt), mtm);
        prompt_field.setFont(Some(&self.ivars().theme.font));
        prompt_field.sizeToFit();
        let prompt_width = if prompt.is_empty() {
            0.0
        } else {
            (prompt_field.frame().size.width + 16.0).min(width / 2.0)
        };

        let panel = PickerPanel::new(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)),
            mtm,
        );
        // SAFETY: The panel is retained in the delegate instead of by an NSWindowController.
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setMovableByWindowBackground(false);
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(false);
        panel.setHasShadow(false);
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&self.ivars().theme.background));
        panel.setLevel(NSFloatingWindowLevel);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Transient,
        );
        panel.setDelegate(Some(ProtocolObject::from_ref(self)));

        let content = panel.contentView().expect("panel must have a content view");

        prompt_field.setFrame(NSRect::new(
            NSPoint::new(0.0, list_height),
            NSSize::new(prompt_width, query_height),
        ));
        prompt_field.setTextColor(Some(&self.ivars().theme.prompt));
        prompt_field.setDrawsBackground(true);
        prompt_field.setBackgroundColor(Some(&self.ivars().theme.prompt_background));

        let field = NSTextField::textFieldWithString(&NSString::from_str(&search), mtm);
        field.setFrame(NSRect::new(
            NSPoint::new(prompt_width, list_height),
            NSSize::new(width - prompt_width, query_height),
        ));
        field.setFont(Some(&self.ivars().theme.font));
        field.setTextColor(Some(&self.ivars().theme.text));
        field.setDrawsBackground(false);
        field.setBordered(false);
        field.setBezeled(false);
        field.setFocusRingType(NSFocusRingType::None);
        // SAFETY: Delegate is retained for the full application run and the property is weak.
        unsafe { field.setDelegate(Some(ProtocolObject::from_ref(self))) };

        let table = NSTableView::new(mtm);
        table.setHeaderView(None);
        table.setStyle(NSTableViewStyle::Plain);
        table.setRowSizeStyle(NSTableViewRowSizeStyle::Custom);
        table.setRowHeight(row_height);
        table.setIntercellSpacing(NSSize::new(0.0, 0.0));
        table.setBackgroundColor(&NSColor::clearColor());
        table.setSelectionHighlightStyle(NSTableViewSelectionHighlightStyle::None);
        table.setAllowsEmptySelection(false);
        table.setAllowsMultipleSelection(false);
        table.setAllowsColumnSelection(false);
        table.setAllowsTypeSelect(false);
        table.setFocusRingType(NSFocusRingType::None);
        table.setFrameSize(NSSize::new(width - prompt_width, document_height));
        let column = NSTableColumn::new(mtm);
        column.setWidth(width - prompt_width);
        table.addTableColumn(&column);
        // SAFETY: Delegate is retained for the full application run and both properties are weak.
        unsafe {
            table.setDataSource(Some(ProtocolObject::from_ref(self)));
            table.setDelegate(Some(ProtocolObject::from_ref(self)));
            table.setTarget(Some(self));
            table.setDoubleAction(Some(sel!(accept:)));
        }

        let scroll = NSScrollView::initWithFrame(
            NSScrollView::alloc(mtm),
            NSRect::new(
                NSPoint::new(prompt_width, 0.0),
                NSSize::new(width - prompt_width, list_height),
            ),
        );
        scroll.setBorderType(NSBorderType::NoBorder);
        scroll.setAutomaticallyAdjustsContentInsets(false);
        scroll.setContentInsets(zero_insets());
        scroll.setScrollerInsets(zero_insets());
        scroll.setDrawsBackground(false);
        scroll.setHasVerticalScroller(false);
        scroll.setDocumentView(Some(&table));
        let clip = scroll.contentView();
        clip.setAutomaticallyAdjustsContentInsets(false);
        clip.setContentInsets(zero_insets());
        clip.setBoundsOrigin(NSPoint::new(0.0, 0.0));

        content.addSubview(&prompt_field);
        content.addSubview(&field);
        content.addSubview(&scroll);

        self.ivars().field.set(field).expect("field set once");
        self.ivars().table.set(table).expect("table set once");
        self.ivars().panel.set(panel).expect("panel set once");
        self.refresh_table();

        let app = NSApplication::sharedApplication(mtm);
        // The modern activate() API does not reliably focus an unbundled CLI process.
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        let panel = self.ivars().panel.get().unwrap();
        panel.center();
        panel.makeKeyAndOrderFront(None);
        let field = self.ivars().field.get().unwrap();
        panel.makeFirstResponder(Some(field));
        if let Some(editor) = field
            .currentEditor()
            .and_then(|editor| editor.downcast::<NSTextView>().ok())
        {
            editor.setInsertionPointColor(Some(&NSColor::clearColor()));
        }
    }

    fn row_view(&self, row: usize) -> Option<Retained<NSView>> {
        let model = self.ivars().model.borrow();
        let matched = model.matches.get(row)?;
        let text = &model.entries[matched.original];
        let selected = model.selected == Some(row);
        let indices = &matched.indices;
        let ns_text = NSString::from_str(text);
        let attributed =
            NSMutableAttributedString::initWithString(NSMutableAttributedString::alloc(), &ns_text);
        let full_range = NSRange::new(0, text.encode_utf16().count());
        let theme = &self.ivars().theme;
        let base_color = if selected {
            &theme.selection_text
        } else {
            &theme.text
        };
        let match_color = if selected {
            &theme.selection_text
        } else {
            &theme.matched
        };

        // SAFETY: Each value is the documented Objective-C type for its attribute key.
        unsafe {
            attributed.addAttribute_value_range(NSFontAttributeName, &theme.font, full_range);
            attributed.addAttribute_value_range(
                NSForegroundColorAttributeName,
                base_color,
                full_range,
            );
            for range in utf16_ranges(text, indices) {
                attributed.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    match_color,
                    range,
                );
            }
        }

        let label = NSTextField::labelWithAttributedString(&attributed, self.mtm());
        label.setDrawsBackground(selected);
        if selected {
            label.setBackgroundColor(Some(&theme.selection));
        }
        Some(label.into_super().into_super())
    }

    fn update_query(&self) {
        let query = self
            .ivars()
            .field
            .get()
            .expect("field must be initialized")
            .stringValue()
            .to_string();
        {
            let mut model = self.ivars().model.borrow_mut();
            model.matches = filter_entries(&model.entries, &query, model.options.sort);
            model.selected = (!model.matches.is_empty()).then_some(0);
        }
        self.refresh_table();
    }

    fn refresh_table(&self) {
        let table = self.ivars().table.get().expect("table must be initialized");
        let selected = self.ivars().model.borrow().selected;
        self.ivars().refreshing_table.set(true);
        table.reloadData();
        if let Some(scroll) = table.enclosingScrollView() {
            scroll.contentView().setBoundsOrigin(NSPoint::new(0.0, 0.0));
        }
        match selected {
            Some(row) => {
                let indexes = NSIndexSet::indexSetWithIndex(row);
                table.selectRowIndexes_byExtendingSelection(&indexes, false);
                table.scrollRowToVisible(row as isize);
            }
            // SAFETY: nil is a valid sender for deselectAll:.
            None => unsafe { table.deselectAll(None) },
        }
        self.ivars().refreshing_table.set(false);
    }

    fn move_selection(&self, delta: isize) {
        let next = {
            let model = self.ivars().model.borrow();
            let len = model.matches.len();
            if len == 0 {
                return;
            }
            let current = model.selected.unwrap_or(0) as isize;
            (current + delta).rem_euclid(len as isize) as usize
        };
        self.ivars().model.borrow_mut().selected = Some(next);
        self.refresh_table();
    }

    fn accept(&self) {
        let selection = {
            let model = self.ivars().model.borrow();
            model
                .selected
                .and_then(|row| model.matches.get(row))
                .map(|matched| matched.original)
        };
        let result = selection.map(Output::Selection).or_else(|| {
            self.ivars().model.borrow().entries.is_empty().then(|| {
                Output::Input(
                    self.ivars()
                        .field
                        .get()
                        .expect("field must be initialized")
                        .stringValue()
                        .to_string(),
                )
            })
        });
        if result.is_some() {
            self.finish(result);
        }
    }

    fn finish(&self, result: Option<Output>) {
        if self.ivars().finished.replace(true) {
            return;
        }
        self.ivars().model.borrow_mut().result = result;
        if let Some(panel) = self.ivars().panel.get() {
            panel.orderOut(None);
        }
        NSApplication::sharedApplication(self.mtm()).stop(None);
    }
}

fn utf16_ranges(text: &str, indices: &[u32]) -> Vec<NSRange> {
    let mut ranges = Vec::with_capacity(indices.len());
    let mut utf16_offset = 0;
    let mut next = indices.iter().copied().peekable();
    for (char_index, character) in text.chars().enumerate() {
        let length = character.len_utf16();
        if next
            .peek()
            .is_some_and(|index| *index as usize == char_index)
        {
            ranges.push(NSRange::new(utf16_offset, length));
            next.next();
        }
        utf16_offset += length;
    }
    ranges
}

fn zero_insets() -> NSEdgeInsets {
    NSEdgeInsets {
        top: 0.0,
        left: 0.0,
        bottom: 0.0,
        right: 0.0,
    }
}

fn install_edit_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let main = NSMenu::new(mtm);
    let edit_root = NSMenuItem::new(mtm);
    let edit = NSMenu::new(mtm);
    for (title, action, key) in [
        ("Undo", sel!(undo:), "z"),
        ("Cut", sel!(cut:), "x"),
        ("Copy", sel!(copy:), "c"),
        ("Paste", sel!(paste:), "v"),
        ("Select All", sel!(selectAll:), "a"),
    ] {
        // SAFETY: Each selector is a standard action dispatched through the responder chain.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            )
        };
        edit.addItem(&item);
    }
    edit_root.setSubmenu(Some(&edit));
    main.addItem(&edit_root);
    app.setMainMenu(Some(&main));
}

pub(crate) fn run(options: Options, entries: &[String]) -> Result<Option<Output>, String> {
    let mtm = MainThreadMarker::new().ok_or("AppKit must run on the main thread")?;
    let app = NSApplication::sharedApplication(mtm);
    let theme = Theme::new(&options, mtm)?;
    let delegate = Delegate::new(Model::new(options, entries), theme, mtm);

    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    install_edit_menu(&app, mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();

    let result = delegate.ivars().model.borrow_mut().result.take();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_character_indices_to_utf16_ranges() {
        let ranges = utf16_ranges("󰀻 Safari", &[0, 2, 3]);
        assert_eq!(
            ranges,
            vec![NSRange::new(0, 2), NSRange::new(3, 1), NSRange::new(4, 1)]
        );
    }

    #[test]
    fn list_viewport_fits_short_lists_and_caps_long_lists() {
        assert_eq!(list_heights(3, 8, 22.0), (66.0, 66.0));
        assert_eq!(list_heights(10, 8, 22.0), (176.0, 220.0));
    }
}
