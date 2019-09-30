use syntax::ast::{self, GenericBound, Ident, Item, ItemKind, Mutability,
                  TraitObjectSyntax, TyKind, VisibilityKind};
use syntax::attr::check_builtin_macro_attribute;
use syntax::ext::base::{Annotatable, ExtCtxt};
use syntax::ptr::P;
use syntax::symbol::{kw, sym, Symbol};
use syntax_pos::Span;

pub fn expand(
    ecx: &mut ExtCtxt<'_>,
    _span: Span,
    meta_item: &ast::MetaItem,
    item: Annotatable,
) -> Vec<Annotatable> {
    check_builtin_macro_attribute(ecx, meta_item, sym::global_allocator);

    let not_static = |item: Annotatable| {
        ecx.parse_sess.span_diagnostic.span_err(item.span(), "allocators must be statics");
        vec![item]
    };
    let item = match item {
        Annotatable::Item(item) => match item.node {
            ItemKind::Static(..) => item,
            _ => return not_static(Annotatable::Item(item)),
        }
        _ => return not_static(item),
    };

    let span = ecx.with_def_site_ctxt(item.span);

    let global_alloc = global_alloc_defn(ecx, span, &item.ident);

    // Return the original item and the new methods.
    vec![Annotatable::Item(item), Annotatable::Item(global_alloc)]
}

// This basically expands into:
//
// ```
// extern_existential::extern_existential! {
//     #[rustc_std_internal_symbol]
//     extern existential type __rust_global_alloc: GlobalAlloc = #{ident};
// }
// ```
fn global_alloc_defn(cx: &mut ExtCtxt<'_>, span: Span, ident: &Ident) -> P<Item> {
    let attr1 = cx.meta_word(span, sym::rustc_std_internal_symbol);
    let attr2 = cx.meta_word(span, sym::no_mangle);
    let attr3 = cx.meta_list_item_word(span, Symbol::intern("non_upper_case_globals"));
    let attr3 = cx.meta_list(span, sym::allow, vec![attr3]);
    let attrs = vec![cx.attribute(attr1), cx.attribute(attr2), cx.attribute(attr3)];

    let trait_object = vec![
        cx.trait_bound(cx.path(span, cx.std_path(&[
            Symbol::intern("alloc"),
            Symbol::intern("GlobalAlloc"),
        ]))),
        cx.trait_bound(cx.path(span, cx.std_path(&[
            Symbol::intern("marker"),
            Symbol::intern("Send"),
        ]))),
        cx.trait_bound(cx.path(span, cx.std_path(&[
            Symbol::intern("marker"),
            Symbol::intern("Sync"),
        ]))),
        GenericBound::Outlives(
            cx.lifetime(span, Ident::new(kw::StaticLifetime, span))
        ),
    ];
    let trait_object = P(ast::Ty {
        id: ast::DUMMY_NODE_ID,
        span,
        node: TyKind::TraitObject(trait_object, TraitObjectSyntax::Dyn),
    });
    let ty = cx.ty_rptr(span, trait_object, None, Mutability::Immutable);

    let val = cx.expr_addr_of(span, cx.expr_ident(span, *ident));

    let mut item = cx.item(
        span,
        Ident::from_str("__rust_global_alloc"),
        attrs,
        ItemKind::Static(ty, Mutability::Immutable, val),
    );
    item.vis.node = VisibilityKind::Public;
    item
}
