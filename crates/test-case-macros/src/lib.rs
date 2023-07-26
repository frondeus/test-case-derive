extern crate proc_macro;

use proc_macro::TokenStream;

use proc_macro2::Span as Span2;
use syn::{parse_macro_input, ItemFn};

use quote::quote;
use syn::parse_quote;
use syn::spanned::Spanned;
use test_case_core::{TestCase, TestMatrix};

/// Generates tests for given set of data
///
/// In general, test case consists of four elements:
///
/// 1. _(Required)_ Arguments passed to test body
/// 2. _(Optional)_ Expected result
/// 3. _(Optional)_ Test case description
/// 4. _(Required)_ Test body
///
///  When _expected result_ is provided, it is compared against the actual value generated with _test body_ using `assert_eq!`.
/// _Test cases_ that don't provide _expected result_ should contain custom assertions within _test body_ or return `Result` similar to `#[test]` macro.
#[proc_macro_attribute]
#[proc_macro_error::proc_macro_error]
pub fn test_case(args: TokenStream, input: TokenStream) -> TokenStream {
    let test_case = parse_macro_input!(args as TestCase);
    let mut item = parse_macro_input!(input as ItemFn);

    let mut test_cases = vec![(test_case, Span2::call_site())];

    match expand_additional_test_case_macros(&mut item) {
        Ok(cases) => test_cases.extend(cases),
        Err(err) => return err.into_compile_error().into(),
    }

    render_test_cases(&test_cases, item)
}

#[proc_macro_attribute]
#[proc_macro_error::proc_macro_error]
pub fn test_matrix(args: TokenStream, input: TokenStream) -> TokenStream {
    let matrix = parse_macro_input!(args as TestMatrix);
    let mut item = parse_macro_input!(input as ItemFn);

    let mut test_cases = expand_test_matrix(&matrix, Span2::call_site());

    match expand_additional_test_case_macros(&mut item) {
        Ok(cases) => test_cases.extend(cases),
        Err(err) => return err.into_compile_error().into(),
    }

    render_test_cases(&test_cases, item)
}

fn expand_test_matrix(matrix: &TestMatrix, span: Span2) -> Vec<(TestCase, Span2)> {
    matrix.cases().map(|c| (c, span)).collect()
}

fn expand_additional_test_case_macros(item: &mut ItemFn) -> syn::Result<Vec<(TestCase, Span2)>> {
    let mut additional_cases = vec![];
    let mut attrs_to_remove = vec![];
    let legal_test_case_names = [
        parse_quote!(test_case),
        parse_quote!(test_case::test_case),
        parse_quote!(test_case::case),
        parse_quote!(case),
    ];
    let legal_test_matrix_names = [
        parse_quote!(test_matrix),
        parse_quote!(test_case::test_matrix),
    ];

    for (idx, attr) in item.attrs.iter().enumerate() {
        if legal_test_case_names.contains(&attr.path) {
            let test_case = match attr.parse_args::<TestCase>() {
                Ok(test_case) => test_case,
                Err(err) => {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("cannot parse test_case arguments: {err}"),
                    ))
                }
            };
            additional_cases.push((test_case, attr.span()));
            attrs_to_remove.push(idx);
        } else if legal_test_matrix_names.contains(&attr.path) {
            let test_matrix = match attr.parse_args::<TestMatrix>() {
                Ok(test_matrix) => test_matrix,
                Err(err) => {
                    return Err(syn::Error::new(
                        attr.span(),
                        format!("cannot parse test_matrix arguments: {err}"),
                    ))
                }
            };
            additional_cases.extend(expand_test_matrix(&test_matrix, attr.span()));
            attrs_to_remove.push(idx);
        }
    }

    for i in attrs_to_remove.into_iter().rev() {
        item.attrs.swap_remove(i);
    }

    Ok(additional_cases)
}

#[allow(unused_mut)]
fn render_test_cases(test_cases: &[(TestCase, Span2)], mut item: ItemFn) -> TokenStream {
    let mut rendered_test_cases = vec![];

    for (test_case, span) in test_cases {
        rendered_test_cases.push(test_case.render(item.clone(), *span));
    }

    let mod_name = item.sig.ident.clone();

    // We don't want any external crate to alter main fn code, we are passing attributes to each sub-function anyway
    item.attrs.retain(|attr| {
        attr.path
            .get_ident()
            .map(|ident| ident == "allow")
            .unwrap_or(false)
    });

    let output = quote! {
        #[allow(unused_attributes)]
        #item

        #[cfg(test)]
        mod #mod_name {
            #[allow(unused_imports)]
            use super::*;

            #(#rendered_test_cases)*
        }
    };

    output.into()
}
