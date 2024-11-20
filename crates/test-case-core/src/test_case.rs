use crate::comment::TestCaseComment;
use crate::expr::{TestCaseExpression, TestCaseResult};
use crate::utils::fmt_syn;
use proc_macro2::{Span as Span2, TokenStream as TokenStream2};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_quote, Attribute, Error, Expr, Ident, ItemFn, ReturnType, Token};

// Check whether given attribute is a test attribute of forms:
// * `#[test]`
// * `#[core::prelude::*::test]` or `#[::core::prelude::*::test]`
// * `#[std::prelude::*::test]` or `#[::std::prelude::*::test]`
fn is_test_attribute(attr: &Attribute) -> bool {
    let path = match &attr.meta {
        syn::Meta::Path(path) => path,
        _ => return false,
    };
    const CANDIDATES_LEN: usize = 4;

    let candidates: [[&str; CANDIDATES_LEN]; 2] = [
        ["core", "prelude", "*", "test"],
        ["std", "prelude", "*", "test"],
    ];
    if path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].arguments.is_none()
        && path.segments[0].ident == "test"
    {
        return true;
    } else if path.segments.len() != candidates[0].len() {
        return false;
    }
    candidates.into_iter().any(|segments| {
        path.segments.iter().zip(segments).all(|(segment, path)| {
            segment.arguments.is_none() && (path == "*" || segment.ident == path)
        })
    })
}

#[derive(Debug)]
pub struct TestCase {
    args: Punctuated<Expr, Token![,]>,
    expression: Option<TestCaseExpression>,
    name: Ident,
}

impl Parse for TestCase {
    fn parse(input: ParseStream) -> Result<Self, Error> {
        let args = Punctuated::parse_separated_nonempty_with(input, Expr::parse)?;
        let expression = (!input.is_empty()).then(|| input.parse()).transpose();
        let comment = (!input.is_empty()).then(|| input.parse()).transpose();
        // if both are errors, pick the expression error since it is more likely to be informative.
        //
        // TODO(https://github.com/frondeus/test-case/issues/135): avoid Result::ok entirely.
        let (expression, comment) = match (expression, comment) {
            (Err(expression), Err(_comment)) => return Err(expression),
            (expression, comment) => (expression.ok().flatten(), comment.ok().flatten()),
        };

        Ok(Self::new_from_parsed(args, expression, comment))
    }
}
impl TestCase {
    pub(crate) fn new<I: IntoIterator<Item = Expr>>(
        args: I,
        expression: Option<TestCaseExpression>,
        comment: Option<TestCaseComment>,
    ) -> Self {
        Self::new_from_parsed(args.into_iter().collect(), expression, comment)
    }

    pub(crate) fn new_from_parsed(
        args: Punctuated<Expr, Token![,]>,
        expression: Option<TestCaseExpression>,
        comment: Option<TestCaseComment>,
    ) -> Self {
        let name = Self::test_case_name_ident(args.iter(), expression.as_ref(), comment.as_ref());

        Self {
            args,
            expression,
            name,
        }
    }

    pub(crate) fn new_with_prefixed_name<I: IntoIterator<Item = Expr>>(
        args: I,
        expression: Option<TestCaseExpression>,
        prefix: &str,
    ) -> Self {
        let parsed_args = args.into_iter().collect::<Punctuated<Expr, Token![,]>>();
        let name = Self::prefixed_test_case_name(parsed_args.iter(), expression.as_ref(), prefix);

        Self {
            args: parsed_args,
            expression,
            name,
        }
    }

    pub fn test_case_name(&self) -> Ident {
        // The clone is kind of annoying here, but because this is behind a reference, we must clone
        // to preserve the signature without a breaking change
        // TODO: return a reference?
        self.name.clone()
    }

    pub fn render(&self, mut item: ItemFn, origin_span: Span2) -> TokenStream2 {
        let item_name = item.sig.ident.clone();
        let arg_values = self.args.iter();
        let test_case_name = {
            let mut test_case_name = self.test_case_name();
            test_case_name.set_span(origin_span);
            test_case_name
        };

        let mut attrs = self
            .expression
            .as_ref()
            .map(|expr| expr.attributes())
            .unwrap_or_default();

        attrs.push(parse_quote! { #[allow(clippy::bool_assert_comparison)] });
        attrs.append(&mut item.attrs);

        let (mut signature, body) = if item.sig.asyncness.is_some() {
            (
                quote! { async },
                quote! { let _result = super::#item_name(#(#arg_values),*).await; },
            )
        } else {
            let qualified_test_attr = parse_quote! { #[::core::prelude::v1::test] };
            if let Some(attr) = attrs.iter().find(|attr| is_test_attribute(attr)) {
                let msg = "second test attribute is supplied, consider removing or changing the order of your test attributes";
                return Error::new_spanned(attr, msg).into_compile_error();
            }
            attrs.push(qualified_test_attr);
            (
                TokenStream2::new(),
                quote! { let _result = super::#item_name(#(#arg_values),*); },
            )
        };

        let expected = if let Some(expr) = self.expression.as_ref() {
            attrs.extend(expr.attributes());

            signature.extend(quote! { fn #test_case_name() });

            if let TestCaseResult::Panicking(_) = expr.result {
                TokenStream2::new()
            } else {
                expr.assertion()
            }
        } else {
            signature.extend(if let ReturnType::Type(_, typ) = item.sig.output {
                quote! { fn #test_case_name() -> #typ }
            } else {
                quote! { fn #test_case_name() }
            });

            quote! { _result }
        };

        quote! {
            #(#attrs)*
            #signature {
                #body
                #expected
            }
        }
    }

    fn test_case_name_ident<'a, I: Iterator<Item = &'a Expr>>(
        args: I,
        expression: Option<&TestCaseExpression>,
        comment: Option<&TestCaseComment>,
    ) -> Ident {
        let desc = Self::test_case_name_string(args, expression, comment);

        crate::utils::escape_test_name(desc)
    }

    fn prefixed_test_case_name<'a, I: Iterator<Item = &'a Expr>>(
        args: I,
        expression: Option<&TestCaseExpression>,
        prefix: &str,
    ) -> Ident {
        let generated_name = Self::test_case_name_string(args, expression, None);
        let full_desc = format!("{prefix}_{generated_name}");

        crate::utils::escape_test_name(full_desc)
    }

    fn test_case_name_string<'a, I: Iterator<Item = &'a Expr>>(
        args: I,
        expression: Option<&TestCaseExpression>,
        comment: Option<&TestCaseComment>,
    ) -> String {
        comment
            .as_ref()
            .map(|item| item.comment.value())
            .unwrap_or_else(|| {
                let mut acc = String::new();
                for arg in args {
                    acc.push_str(&fmt_syn(&arg));
                    acc.push('_');
                }
                acc.push_str("expects");
                if let Some(expression) = expression {
                    acc.push(' ');
                    acc.push_str(&expression.to_string())
                }
                acc
            })
    }
}
