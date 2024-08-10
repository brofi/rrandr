use glib::{wrapper, Object};
use gtk::{glib, Widget};

mod imp {
    use std::cell::Cell;

    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::ObjectSubclass;
    use glib::{derived_properties, object_subclass, GString, Properties};
    use gtk::prelude::{BoxExt, ObjectExt, WidgetExt};
    use gtk::subclass::prelude::{DerivedObjectProperties, ObjectSubclassExt};
    use gtk::subclass::widget::{WidgetClassExt, WidgetImpl};
    use gtk::{
        glib, AccessibleRole, Align, BinLayout, Box, IconLookupFlags, IconTheme, Image, Label,
        Orientation, Widget,
    };

    use crate::window::SPACING;

    const NO_ICON: &str = "image-missing";

    #[derive(Properties)]
    #[properties(wrapper_type = super::IconText)]
    pub struct IconText {
        hbox: Box,
        #[property(get, set, construct_only)]
        prefer_icon_only: Cell<bool>,
        #[property(name = "icon-name", get = Self::icon_name, set = Self::set_icon_name, type = GString)]
        image: Image,
        #[property(get = Self::label, set = Self::set_label, type = GString)]
        label: Label,
    }

    impl Default for IconText {
        fn default() -> Self {
            Self {
                hbox: Box::builder()
                    .orientation(Orientation::Horizontal)
                    .halign(Align::Center)
                    .build(),
                image: Image::builder()
                    .icon_name(NO_ICON)
                    .valign(Align::Center)
                    .visible(false)
                    .build(),
                label: Label::builder().use_underline(true).visible(false).build(),
                prefer_icon_only: Cell::default(),
            }
        }
    }

    #[object_subclass]
    impl ObjectSubclass for IconText {
        type ParentType = Widget;
        type Type = super::IconText;

        const NAME: &'static str = "IconText";

        fn class_init(klass: &mut Self::Class) {
            klass.set_layout_manager_type::<BinLayout>();
            klass.set_accessible_role(AccessibleRole::Group);
        }
    }

    #[derived_properties]
    impl ObjectImpl for IconText {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_hexpand(false);
            self.label.set_mnemonic_widget(obj.parent().as_ref());
            self.hbox.append(&self.image);
            self.hbox.append(&self.label);
            self.hbox.set_parent(&*obj);
        }

        fn dispose(&self) { self.hbox.unparent(); }
    }

    impl WidgetImpl for IconText {}

    impl IconText {
        fn label(&self) -> GString { self.label.text() }

        fn set_label(&self, text: &str) {
            self.label.set_label(text);
            if !self.prefer_icon_only.get() {
                self.hbox.set_spacing(SPACING.into());
                self.label.set_visible(true);
            }
        }

        fn icon_name(&self) -> GString {
            self.image
                .icon_name()
                .unwrap_or_else(|| panic!("icon name should be '{NO_ICON}' by default"))
        }

        fn set_icon_name(&self, icon_name: &str) {
            if self.icon_exists(icon_name) {
                self.image.set_icon_name(Some(icon_name));
                self.image.set_visible(true);
            } else {
                self.hbox.set_spacing(0);
                self.label.set_visible(true);
            }
        }

        fn icon_exists(&self, icon_name: &str) -> bool {
            let obj = self.obj();
            IconTheme::for_display(&obj.display())
                .lookup_icon(
                    icon_name,
                    &[],
                    48,
                    1,
                    obj.direction(),
                    IconLookupFlags::FORCE_SYMBOLIC,
                )
                .icon_name()
                .is_some_and(|p| p.to_str().is_some_and(|name| name != NO_ICON))
        }
    }
}

wrapper! {
    pub struct IconText(ObjectSubclass<imp::IconText>) @extends Widget;
}

impl IconText {
    pub fn new(prefer_icon_only: bool) -> Self {
        Object::builder().property("prefer-icon-only", prefer_icon_only).build()
    }
}

impl Default for IconText {
    fn default() -> Self { Object::new() }
}
