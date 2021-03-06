use font::{Font, FontDescriptor, FontGroup, FontStyle, SelectorPlatformIdentifier};
use font::{SelectorStubDummy, SpecifiedFontStyle, UsedFontStyle};
use font_list::FontList;
use native::FontHandle;
use util::cache::Cache;
use util::cache::MonoCache;

use azure::azure_hl::BackendType;
use core::dvec::DVec;
use core::hashmap::linear::LinearMap;
use core::hashmap::linear;

#[cfg(target_os = "macos")]
use quartz;
#[cfg(target_os = "linux")]
use freetype_impl;
use font_context;

// TODO(Issue #164): delete, and get default font from font list
const TEST_FONT: [u8 * 33004] = include_bin!("JosefinSans-SemiBold.ttf");

fn test_font_bin() -> ~[u8] {
    return vec::from_fn(33004, |i| TEST_FONT[i]);
}

// TODO(Rust #3934): creating lots of new dummy styles is a workaround
// for not being able to store symbolic enums in top-level constants.
pub fn dummy_style() -> FontStyle {
    use font::FontWeight300;
    return FontStyle {
        pt_size: 20f,
        weight: FontWeight300,
        italic: false,
        oblique: false,
        families: ~"serif, sans-serif",
    }
}

#[cfg(target_os = "macos")]
type FontContextHandle/& = quartz::font_context::QuartzFontContextHandle;

#[cfg(target_os = "linux")]
type FontContextHandle/& = freetype_impl::font_context::FreeTypeFontContextHandle;

pub trait FontContextHandleMethods {
    pure fn clone(&const self) -> FontContextHandle;
    fn create_font_from_identifier(~str, UsedFontStyle) -> Result<FontHandle, ()>;
}

// TODO(Issue #163): this is a workaround for static methods, traits,
// and typedefs not working well together. It should be removed.
pub impl FontContextHandle {
    #[cfg(target_os = "macos")]
    static pub fn new() -> FontContextHandle {
        quartz::font_context::QuartzFontContextHandle::new()
    }

    #[cfg(target_os = "linux")]
    static pub fn new() -> FontContextHandle {
        freetype_impl::font_context::FreeTypeFontContextHandle::new()
    }
}

#[allow(non_implicitly_copyable_typarams)]
pub struct FontContext {
    instance_cache: MonoCache<FontDescriptor, @Font>,
    font_list: Option<FontList>, // only needed by layout
    handle: FontContextHandle,
    backend: BackendType,
    generic_fonts: LinearMap<~str,~str>,
}

#[allow(non_implicitly_copyable_typarams)]
pub impl FontContext {
    static fn new(backend: BackendType, needs_font_list: bool) -> FontContext {
        let handle = FontContextHandle::new();
        let font_list = if needs_font_list { Some(FontList::new(&handle)) } else { None };

        // TODO: Allow users to specify these.
        let mut generic_fonts = linear::linear_map_with_capacity(5);
        generic_fonts.insert(~"serif", ~"Times New Roman");
        generic_fonts.insert(~"sans-serif", ~"Arial");
        generic_fonts.insert(~"cursive", ~"Apple Chancery");
        generic_fonts.insert(~"fantasy", ~"Papyrus");
        generic_fonts.insert(~"monospace", ~"Menlo");

        FontContext { 
            // TODO(Rust #3902): remove extraneous type parameters once they are inferred correctly.
            instance_cache:
                Cache::new::<FontDescriptor,@Font,MonoCache<FontDescriptor,@Font>>(10),
            font_list: font_list,
            handle: handle,
            backend: backend,
            generic_fonts: generic_fonts,
        }
    }

    priv pure fn get_font_list(&self) -> &self/FontList {
        option::get_ref(&self.font_list)
    }

    fn get_resolved_font_for_style(style: &SpecifiedFontStyle) -> @FontGroup {
        // TODO(Issue #178, E): implement a cache of FontGroup instances.
        self.create_font_group(style)
    }

    fn get_font_by_descriptor(desc: &FontDescriptor) -> Result<@Font, ()> {
        match self.instance_cache.find(desc) {
            Some(f) => Ok(f),
            None => { 
                let result = self.create_font_instance(desc);
                match result {
                    Ok(font) => {
                        self.instance_cache.insert(desc, font);
                    }, _ => {}
                };
                result
            }
        }
    }

    priv fn transform_family(&self, family: &str) -> ~str {
        // FIXME: Need a find_like() in LinearMap.
        let family = family.to_str();
        debug!("(transform family) searching for `%s`", family);
        match self.generic_fonts.find(&family) {
            None => family,
            Some(mapped_family) => copy *mapped_family
        }
    }

    // TODO:(Issue #196): cache font groups on the font context.
    priv fn create_font_group(style: &SpecifiedFontStyle) -> @FontGroup {
        let fonts = DVec();

        debug!("(create font group) --- starting ---");

        // TODO(Issue #193): make iteration over 'font-family' more robust.
        for str::split_char_each(style.families, ',') |family| {
            let family_name = str::trim(family);
            let transformed_family_name = self.transform_family(family_name);
            debug!("(create font group) transformed family is `%s`", transformed_family_name);

            let list = self.get_font_list();

            let result = list.find_font_in_family(transformed_family_name, style);
            let mut found = false;
            do result.iter |font_entry| {
                found = true;
                // TODO(Issue #203): route this instantion through FontContext's Font instance cache.
                let instance = Font::new_from_existing_handle(&self, &font_entry.handle, style, self.backend);
                do result::iter(&instance) |font: &@Font| { fonts.push(*font); }
            };

            if !found {
                debug!("(create font group) didn't find `%s`", transformed_family_name);
            }
        }

        // TODO(Issue #194): *always* attach a fallback font to the
        // font list, so that this assertion will never fail.

        // assert fonts.len() > 0;
        if fonts.len() == 0 {
            let desc = FontDescriptor::new(font_context::dummy_style(), SelectorStubDummy);
            debug!("(create font group) trying descriptor `%?`", desc);
            match self.get_font_by_descriptor(&desc) {
                Ok(instance) => fonts.push(instance),
                Err(()) => {}
            }
        }
        assert fonts.len() > 0;
        // TODO(Issue #179): Split FontStyle into specified and used styles
        let used_style = copy *style;

        debug!("(create font group) --- finished ---");

        @FontGroup::new(style.families.to_managed(), &used_style, dvec::unwrap(fonts))
    }

    priv fn create_font_instance(desc: &FontDescriptor) -> Result<@Font, ()> {
        return match &desc.selector {
            &SelectorStubDummy => {
                Font::new_from_buffer(&self, test_font_bin(), &desc.style, self.backend)
            },
            // TODO(Issue #174): implement by-platform-name font selectors.
            &SelectorPlatformIdentifier(ref identifier) => { 
                let result_handle = self.handle.create_font_from_identifier(copy *identifier,
                                                                            copy desc.style);
                result::chain(result_handle, |handle| {
                    Ok(Font::new_from_adopted_handle(&self,
                                                     handle,
                                                     &desc.style,
                                                     self.backend))
                })
            }
        };
    }
}
