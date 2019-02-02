#![recursion_limit = "128"]
extern crate proc_macro;

use std::collections::HashMap;
use std::string::ToString;

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use quote::ToTokens;
use syn::parse::Error;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Fields, Ident, Lit, Meta, MetaList,
    MetaNameValue, NestedMeta, Type,
};

// error messages
const INVALID_FORMAT: &str = "Invalid annotation format.";
const MULTIPLE_ANNOT: &str = "Found multiple `#[imgui(...)]` annotations in a single field.";
const STRUCT_SUPPORT: &str = "`#[derive(ImGuiExt)]` only supports structs.";
const UNRECOG_MODE: &str = "Unrecognized mode.";
const UNEXPECTED_PARAM: &str = "Unexpected parameter.";
const FIELD_ALREADY_DEFINED: &str = "Field is already defined.";
const NESTED_BULLET: &str = "Nested `bullet` is not yet implemented. See #0";
const NESTED_INPUTS: &str = "Nested input is not yet implemented. See #0";

macro_rules! tag {
    (
        $(#[$meta:meta])*
        struct $tag:ident {
            fields { $( $field:ident : Lit ,)* },
            optional { $( $opt_field:ident : Option<Lit> ,)* }
        }
    ) => {
        $(#[$meta])*
        struct $tag {
            $( $field : Lit ,)*
            $( $opt_field : Option<Lit> ,)*
        }
        impl $tag {
            fn from_meta_list(list: &MetaList) -> Result<Self, Error> {
                $( let mut $field = None; )*
                $( let mut $opt_field = None; )*
                for param in list.nested.iter() {
                    match param {
                        NestedMeta::Meta(Meta::NameValue(MetaNameValue { ident, lit, .. })) => match ident.to_string().as_str() {
                            //"label" => widget.label = Some(lit.clone()),
                            $( stringify!($opt_field) => {
                                if $opt_field.is_some() {
                                    return Err(Error::new(ident.span(), FIELD_ALREADY_DEFINED));
                                }
                                $opt_field = Some(lit.clone());
                            },)*
                            $( stringify!($field) => {
                                if $field.is_some() {
                                    return Err(Error::new(ident.span(), FIELD_ALREADY_DEFINED));
                                }
                                $field = Some(lit.clone());
                            },)*
                            _ => return Err(Error::new(ident.span(), UNEXPECTED_PARAM)),
                        }
                        // TODO use proper span
                        _ => return Err(Error::new(list.span(), INVALID_FORMAT)),
                    }
                }
                Ok(Self {
                    $( $field : $field.ok_or(Error::new(list.span(), format!("Parameter `{}` missing.", stringify!($field) )))?,)*
                    $( $opt_field,)*
                })
            }
        }

    }
}

#[proc_macro_derive(ImGuiExt, attributes(imgui))]
pub fn imgui_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match impl_derive(&input) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

enum DisplayParam {
    Literal(Lit),
    Ident(Ident),
}

/// `#[imgui(label(label = "...", display = "...", x, y, z))]`
///                                                   ^---,
///                                                idents & literals
#[derive(Default)]
struct Label {
    label: Option<Lit>,
    display: Option<Lit>,
    params: Vec<DisplayParam>,
}

tag! {
    /// `#[imgui(checkbox(label = "..."))]`
    #[derive(Default)]
    struct Checkbox {
        fields {
            // none
        },
        optional {
            label: Option<Lit>,
            catch: Option<Lit>,
        }
    }
}

tag! {
    /// `#[imgui(input(label = "...", step = 1.0, step_fast = 1.0, precision = 3))]`
    #[derive(Default)]
    struct Input {
        fields {
            // none
        },
        optional {
            label: Option<Lit>,
            flags: Option<Lit>,
            step: Option<Lit>,
            step_fast: Option<Lit>,
            precision: Option<Lit>,
            catch: Option<Lit>,
        }
    }
}

tag! {
    /// `#[imgui(slider(label = "...", min = 0.0, max = 4.0, format = "..."))]`
    struct Slider {
        fields {
            min: Lit,
            max: Lit,
        },
        optional {
            label: Option<Lit>,
            format: Option<Lit>,
            power: Option<Lit>,
            catch: Option<Lit>,
        }
    }
}

tag! {
    #[derive(Default)]
    struct Drag {
        fields {
            // none
        },
        optional {
            label: Option<Lit>,
            min: Option<Lit>,
            max: Option<Lit>,
            speed: Option<Lit>,
            power: Option<Lit>,
            format: Option<Lit>,
            catch: Option<Lit>,
        }
    }
}

tag! {
    struct Button {
        fields {
            size: Lit,
        },
        optional {
            label: Option<Lit>,
            catch: Option<Lit>,
        }
    }
}

tag! {
    /// `#[imgui(bullet)]`
    /// `#[imgui(bullet(label = "Bullet list item"))]`
    #[derive(Default)]
    struct Bullet {
        fields {
        },
        optional {
            text: Option<Lit>,
        }
    }
}

tag! {
    #[derive(Default)]
    struct Nested {
        fields {
        },
        optional {
            catch: Option<Lit>,
        }
    }
}

enum Tag {
    Label(Label),
    Checkbox(Checkbox),
    Input(Input),
    Slider(Slider),
    Drag(Drag),
    Button(Button),
    Bullet(Bullet),
    Nested(Nested),

    /// `#[imgui(separator)]`
    Separator,
    /// `#[imgui(new_line)]`
    NewLine,
}

impl Tag {
    fn display(&mut self) -> &mut Label {
        if let &mut Tag::Label(ref mut disp) = self {
            disp
        } else {
            panic!("Unexpected state")
        }
    }
}

fn impl_derive(input: &DeriveInput) -> Result<TokenStream, Error> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let (body, catch_fields) = match input.data {
        Data::Struct(ref body) => struct_body(body.fields.clone()),
        _ => Err(Error::new(input.span(), STRUCT_SUPPORT)),
    }?;

    // crate a new type.
    // It should never generate a collision
    let event_type = Ident::new(&format!("{}ImGuiExt", name.to_string()), input.span());

    Ok(quote! {
        #[derive(Default)]
        pub struct #event_type {
            #catch_fields
        }
        impl #impl_generics imgui_ext::ImGuiExt for #name #ty_generics #where_clause {
            type Events = #event_type;
            fn imgui_ext(ui: &imgui::Ui, ext: &mut Self) -> Self::Events {
                let mut events: Self::Events = Default::default();
                #body
                events
            }
        }
    })
}

/// Adds support to allow multiple imgui tags in a single field:
/// ```
/// struct Demo {
///     #[imgui(drag(...))]
///     x: f32,
///
///     // multiple annotations
///     #[imgui(separator)]
///     #[imgui(slider(...))]
///     #[imgui(input(...))]
///     y: f32,
/// }
#[rustfmt::skip]
fn struct_body(fields: Fields) -> Result<(TokenStream, TokenStream), Error> {
    // TODO FIXME refactor the way input fields are collected
    let mut input: Vec<TokenStream> = vec![];

    let field_body = fields
        .iter()
        .flat_map(|field| {

            // TODO add support for unnamed attributes
            let ident = field.ident.clone().expect("Named field");
            let ty = &field.ty;

            // collect all the imgui attributes
            // we need to check that there is only one.
            let attrs: Vec<Attribute> = field.attrs.iter()
                .filter(|attr| {
                    let ident = Ident::new("imgui", attr.span());
                    attr.path.is_ident(ident)
                })
                .cloned()
                .collect();

            let mut attrs = attrs.into_iter();
            let first = attrs.next();
            let second = attrs.next();

            match (first, second) {
                // No annotations were found.
                // Emmit no sourcecode.
                (None,          None     ) => vec![Ok(TokenStream::new())],

                // There is more than one imgui annotation.
                // Raise a descriptive error pointing to the extra annotation.
                (Some(_),       Some(err)) => vec![Err(Error::new(err.span(), MULTIPLE_ANNOT))],

                // There is a single annotation, as it should.
                // Parse the annotation and emmit the source code for this field
                (Some(attr),    None     ) => {
                    let tags = attr
                        .parse_meta()           // -> Meta
                        .and_then(parse_meta);  // -> Result<Vec<Tag>>

                    match tags {
                        Err(e) => vec![Err(e)],
                        Ok(tags) => tags
                            .into_iter()
                            .map(|tag| emmit_tag_tokens(&ident, &ty, &attr, &tag, &mut input))
                            .collect()
                    }
                },

                _ => unreachable!(),
            }
        })
        .collect::<Result<Vec<_>, Error>>()?;

    let catch_fields = quote! {
        #( #input ),*
        //pub click: bool,
        //pub rem: bool,
    };

    Ok((quote! { #( #field_body );*}, catch_fields))
}

/// meta is the whole (parsed) tag: `#[imgui]` or `#[imgui(...)]`
fn parse_meta(meta: Meta) -> Result<Vec<Tag>, Error> {
    match meta {
        // #[imgui = ...] Nope
        Meta::NameValue(named) => Err(Error::new(named.span(), INVALID_FORMAT)),
        // #[imgui], treated as an empty label
        Meta::Word(_) => Ok(vec![Tag::Label(Label::default())]),
        // #[imgui(meta_list)] (general)
        Meta::List(meta_list) => parse_meta_list(meta_list),
    }
}

/// Parse the inside of `#[imgui(...)]`
///                              ^^^
/// Possible cases:
///   - `#[imgui(foo(...), bar(...))]`
///   - `#[imgui(foo(...),)]`
///   - `#[imgui(foo(...))]`
///   - `#[imgui(label = "...", display = "...", foo, bar)]`
fn parse_meta_list(meta_list: MetaList) -> Result<Vec<Tag>, Error> {
    #[derive(Copy, Clone, Eq, PartialEq)]
    enum State {
        Init,
        Label,
        Tags,
    }

    let mut state = State::Init;
    let mut tags = vec![];

    for nested in meta_list.nested.iter() {
        match (state, nested) {
            (_, NestedMeta::Literal(_)) => return Err(Error::new(meta_list.span(), INVALID_FORMAT)),
            // Parse as a label(...)
            (State::Init, NestedMeta::Meta(Meta::NameValue(MetaNameValue { ident, .. })))
                if ident.to_string() == "label" || ident.to_string() == "display" =>
            {
                tags.push(Tag::Label(parse_label(&meta_list)?));
                // any errors will have been reported by the previous call to `parse_label`.
                // At this point I can break out of the loop.
                break;
            }

            // widgets that can take no parameters
            (s, NestedMeta::Meta(Meta::Word(ident))) if s == State::Init || s == State::Tags => {
                match ident.to_string().as_str() {
                    "separator" => tags.push(Tag::Separator),
                    "new_line" => tags.push(Tag::NewLine),

                    "nested" => tags.push(Tag::Nested(Default::default())),
                    "label" => tags.push(Tag::Label(Default::default())),
                    "checkbox" => tags.push(Tag::Checkbox(Default::default())),
                    "input" => tags.push(Tag::Input(Default::default())),
                    "drag" => tags.push(Tag::Drag(Default::default())),
                    "bullet" => tags.push(Tag::Bullet(Default::default())),
                    _ => return Err(Error::new(meta_list.span(), UNRECOG_MODE)),
                }
                state = State::Tags;
            }
            (s, NestedMeta::Meta(Meta::List(meta_list)))
                if s == State::Init || s == State::Tags =>
            {
                let tag = match meta_list.ident.to_string().as_str() {
                    "separator" => Tag::Separator,
                    "new_line" => Tag::NewLine,

                    "label" => Tag::Label(parse_label(&meta_list)?),
                    "nested" => Tag::Nested(Nested::from_meta_list(meta_list)?),
                    "checkbox" => Tag::Checkbox(Checkbox::from_meta_list(meta_list)?),
                    "input" => Tag::Input(Input::from_meta_list(meta_list)?),
                    "drag" => Tag::Drag(Drag::from_meta_list(meta_list)?),
                    "slider" => Tag::Slider(Slider::from_meta_list(meta_list)?),
                    "button" => Tag::Button(Button::from_meta_list(meta_list)?),

                    // TODO implement nested bullet
                    "bullet" => {
                        if meta_list.nested.len() == 1 {
                            use syn::punctuated::Pair;
                            let span = meta_list.span();
                            match meta_list.nested.first() {
                                Some(Pair::Punctuated(NestedMeta::Meta(Meta::List(_)), _))
                                | Some(Pair::End(NestedMeta::Meta(Meta::List(_)))) => {
                                    return Err(Error::new(span, NESTED_BULLET));
                                }
                                _ => Tag::Bullet(Bullet::from_meta_list(meta_list)?),
                            }
                        } else {
                            Tag::Bullet(Bullet::from_meta_list(meta_list)?)
                        }
                    }
                    _ => return Err(Error::new(meta_list.span(), UNRECOG_MODE)),
                };

                tags.push(tag);
                state = State::Tags;
            }
            _ => panic!(),
        }
    }
    Ok(tags)
}

/// Parse the contents of: `foo(k=v, ...)`
/// It must contain only key=value pairs, otherwise returns an Err.
fn parse_params(params: &MetaList) -> Result<HashMap<String, (Ident, Lit)>, Error> {
    params
        .nested
        .iter()
        .map(|nested| {
            if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { ident, lit, .. })) = nested {
                Ok((ident.to_string(), (ident.clone(), lit.clone())))
            } else {
                Err(Error::new(params.span(), INVALID_FORMAT))
            }
        })
        .collect::<Result<HashMap<_, _>, Error>>()
}

/// Parse the contents of a label tag: `label(label = "...", display = "...", foo, bar)`
/// Asumes that `params.ident` is equal to "label"
fn parse_label(params: &MetaList) -> Result<Label, Error> {
    #[derive(Clone, Copy)]
    enum State {
        Init,
        Display,
    }

    let mut state = State::Init;
    let mut display = Label::default();

    for attr in params.nested.iter() {
        match (state, attr) {
            (State::Display, NestedMeta::Literal(lit)) => {
                display.params.push(DisplayParam::Literal(lit.clone()));
            }

            (State::Display, NestedMeta::Meta(Meta::Word(ident))) => {
                display.params.push(DisplayParam::Ident(ident.clone()));
            }

            (State::Init, NestedMeta::Meta(Meta::NameValue(MetaNameValue { ident, lit, .. })))
                if ident.to_string() == "label" =>
            {
                display.label = Some(lit.clone());
            }

            (State::Init, NestedMeta::Meta(Meta::NameValue(MetaNameValue { ident, lit, .. })))
                if ident.to_string() == "display" =>
            {
                display.display = Some(lit.clone());
                state = State::Display;
            }

            _ => return Err(Error::new(params.span(), INVALID_FORMAT)),
        }
    }

    Ok(display)
}

/// Output source code for a given field, a given attribute, and one of the parsed `Tag`s
///
/// For example, the this annotation: `#[imgui(label(...), input(...))]`
/// produces two tags: `Tag::Label` and `Tag::Input`.
///
/// This function needs to be called twice (once per Tag)
fn emmit_tag_tokens(
    ident: &Ident,
    ty: &Type,
    attr: &Attribute,
    tag: &Tag,
    input: &mut Vec<TokenStream>,
) -> Result<TokenStream, Error> {
    let tokens = match tag {
        Tag::Separator => quote!({ ui.separator() }),
        Tag::NewLine => quote!({ ui.new_line() }),
        Tag::Input(Input {
            label,
            step,
            step_fast,
            precision,
            flags,
            catch,
        }) => {
            let label = match label {
                Some(Lit::Str(stri)) => stri.value(),
                None => ident.to_string(),
                // TODO proper error span
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);
            let mut params = quote! {
                use imgui_ext::input::InputParams as Params;
                use imgui::im_str;
                let mut params = Params {
                    label: im_str!( #label ),
                    precision: None,
                    step: None,
                    step_fast: None,
                    flags: None,
                };
            };

            match step {
                Some(Lit::Float(step)) => params.extend(quote! { params.step = Some(#step); }),
                Some(Lit::Int(step)) => params.extend(quote! { params.step = Some(#step); }),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            match step_fast {
                Some(Lit::Float(step)) => params.extend(quote! { params.step_fast = Some(#step); }),
                Some(Lit::Int(step)) => params.extend(quote! { params.step_fast = Some(#step); }),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            match precision {
                Some(Lit::Int(value)) => params.extend(quote! { params.precision = Some(#value); }),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            match flags {
                Some(Lit::Str(flags)) => {
                    let fn_ident = Ident::new(&flags.value(), flags.span());
                    params.extend(quote! { params.flags = Some( #fn_ident() ); });
                }
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            params.extend(quote!(params));
            let catch = if let Some(Lit::Str(c)) = catch {
                let id = Ident::new(&c.value(), ident.span());
                let q = quote! { events.#id = _ev; };
                input.push(quote! { #id: bool });
                q
            } else {
                quote!()
            };
            quote!({
                use imgui_ext::input::Input;
                let _ev = Input::build(ui, &mut ext.#ident, { #params });
                #catch
            })
        }
        Tag::Drag(Drag {
            label,
            min,
            max,
            speed,
            power,
            format,
            catch,
        }) => {
            let label = match label {
                Some(Lit::Str(stri)) => stri.value(),
                None => ident.to_string(),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);
            let mut params = quote! {
                use imgui_ext::drag::DragParams as Params;
                use imgui::im_str;
                let mut params = Params {
                    label: im_str!( #label ),
                    min: None,
                    max: None,
                    speed: None,
                    power: None,
                    format: None,
                };
            };

            match (min, max) {
                (Some(Lit::Float(min)), Some(Lit::Float(max))) => {
                    params.extend(quote!(params.min = Some(#min);));
                    params.extend(quote!(params.max = Some(#max);));
                }
                (Some(Lit::Int(min)), Some(Lit::Int(max))) => {
                    params.extend(quote!(params.min = Some(#min);));
                    params.extend(quote!(params.max = Some(#max);));
                }
                (Some(Lit::Float(min)), None) => params.extend(quote!(params.min = Some(#min);)),
                (Some(Lit::Int(min)), None) => params.extend(quote!(params.min = Some(#min);)),
                (None, Some(Lit::Float(max))) => params.extend(quote!(params.max = Some(#max);)),
                (None, Some(Lit::Int(max))) => params.extend(quote!(params.max = Some(#max);)),
                (None, None) => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            match speed {
                Some(Lit::Float(value)) => params.extend(quote! { params.speed = Some(#value); }),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }
            match power {
                Some(Lit::Float(value)) => params.extend(quote! { params.power = Some(#value); }),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }
            match format {
                Some(Lit::Str(value)) => {
                    params.extend(quote!(params.format = Some(im_str!(#value));))
                }
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            params.extend(quote!(params));
            quote!({
                use imgui_ext::drag::Drag;
                Drag::build(ui, &mut ext.#ident, { #params });
            })
        }
        Tag::Button(Button { label, size, catch }) => {
            let label = match label {
                Some(Lit::Str(stri)) => stri.value(),
                None => ident.to_string(),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);

            let size_fn = match size {
                Lit::Str(size) => Ident::new(&size.value(), size.span()),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let catch = if let Some(Lit::Str(c)) = catch {
                let id = Ident::new(&c.value(), ident.span());
                let q = quote! { events.#id = _ev; };
                input.push(quote! { #id: bool });
                q
            } else {
                quote!()
            };
            quote! {{
                let _ev = ui.button( imgui::im_str!( #label ), { #size_fn() } );
                #catch
            }}
        }
        Tag::Bullet(Bullet { text }) => {
            let text = match text {
                Some(Lit::Str(text)) => Some(text),
                None => None,
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };

            if let Some(text) = text {
                quote! {{
                    use imgui::im_str;
                    ui.bullet_text( im_str!( #text ));
                }}
            } else {
                quote! { ui.bullet(); }
            }
        }
        Tag::Slider(Slider {
            label,
            min,
            max,
            format,
            power,
            catch,
        }) => {
            let label = match label {
                Some(Lit::Str(stri)) => stri.value(),
                None => ident.to_string(),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);
            let min_max = match (min, max) {
                (Lit::Int(min), Lit::Int(max)) => quote! { min: #min, max: #max },
                (Lit::Float(min), Lit::Float(max)) => quote! { min: #min, max: #max },
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let mut params = quote! {
                use imgui_ext::slider::SliderParams as Params;
                use imgui::im_str;
                let mut params = Params {
                    label: im_str!( #label ),
                    format: None,
                    #min_max,
                    power: None,
                };
            };
            match format {
                Some(Lit::Str(value)) => {
                    params.extend(quote!(params.format = Some( im_str!(#value) );))
                }
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }
            match power {
                Some(Lit::Float(value)) => params.extend(quote!(params.power = Some(#value);)),
                None => {}
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            }

            let catch = if let Some(Lit::Str(c)) = catch {
                let id = Ident::new(&c.value(), ident.span());
                let q = quote! { events.#id = _ev; };
                input.push(quote! { #id: bool });
                q
            } else {
                quote!()
            };

            params.extend(quote!(params));
            quote!({
                use imgui_ext::slider::Slider;
                let _ev = Slider::build(ui, &mut ext.#ident, { #params });
                #catch
            })
        }
        Tag::Checkbox(Checkbox { label, catch }) => {
            let label = match label {
                Some(Lit::Str(lab)) => lab.value(),
                None => ident.to_string(),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);
            let catch = if let Some(Lit::Str(c)) = catch {
                let id = Ident::new(&c.value(), ident.span());
                let q = quote! { events.#id = _ev; };
                input.push(quote! { #id: bool });
                q
            } else {
                quote!()
            };
            quote!({
                use imgui_ext::checkbox::Checkbox;
                use imgui_ext::checkbox::CheckboxParams as Params;
                use imgui::im_str;
                let _ev = Checkbox::build(ui, &mut ext.#ident, Params { label: im_str!(#label) });
                #catch
            })
        }
        Tag::Nested(Nested { catch }) => {
            // TODO catch events
            let catch = if let Some(catch) = catch {
                return Err(Error::new(attr.span(), NESTED_INPUTS));
            /*
            match ty {
                Type::Path(path) => unimplemented!("Nested type input catch."),
                _ => panic!("Invalid field type"),
            }
            */
            } else {
                quote!()
            };
            quote! {{
                use imgui_ext::ImGuiExt;
                let _ev = ImGuiExt::imgui_ext(ui, &mut ext.#ident);
                #catch
            }}
        }
        Tag::Label(Label {
            label,
            display,
            params,
        }) => {
            let label = match label {
                Some(Lit::Str(lab)) => lab.value(),
                None => ident.to_string(),
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };
            let label = Literal::string(&label);

            let display = match display {
                Some(Lit::Str(disp)) => Some(disp.value()),
                None => None,
                _ => return Err(Error::new(attr.span(), INVALID_FORMAT)),
            };

            let display = if let Some(display) = display {
                let literal = Literal::string(display.as_str());
                let params: Vec<_> = params
                    .into_iter()
                    .map(|field| match field {
                        DisplayParam::Literal(lit) => quote!( ext.#ident.#lit ),
                        DisplayParam::Ident(ident) => quote!( ext.#ident.#ident ),
                    })
                    .collect();
                quote!(#literal , #( #params ),*)
            } else {
                // display the variable using the Display trait
                quote!("{}", ext.#ident)
            };

            quote!({
                use imgui::im_str;
                ui.label_text(im_str!(#label), im_str!(#display));
            })
        }
    };

    Ok(tokens)
}