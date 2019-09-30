use rustc::ty::TyCtxt;
use syntax::ext::allocator::AllocatorKind;
use crate::ModuleLlvm;

pub(crate) unsafe fn codegen(_tcx: TyCtxt<'_>, _mods: &mut ModuleLlvm, _kind: AllocatorKind) {}
