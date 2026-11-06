//! §13.3.1: every table entry of one subroutine declaration carries one
//! identity, on both generate expansion paths.

use xezim_core::ast::decl::SubroutineIdent;

fn elaborate(src: &str) -> xezim_core::ElaboratedModule {
    xezim_core::parse_and_elaborate_multi(
        &[src.to_string()],
        Some("top"),
        &[],
        &["top.sv".to_string()],
        &[],
    )
    .expect("elaborate")
    .1
}

fn ident(canonical: &str, instance: &str, scope: &str) -> SubroutineIdent {
    SubroutineIdent {
        canonical: canonical.to_string(),
        instance: instance.to_string(),
        scope: scope.to_string(),
    }
}

const SRC: &str = r#"
package p;
  function int pf(); return 1; endfunction
  task pt(); endtask
endpackage
module m;
  function int mf(); return 1; endfunction
  task mt(); endtask
  if (1) begin : g
    function int f(); return 1; endfunction
    task t(); endtask
    if (1) begin : n
      function int h(); return 1; endfunction
    end
  end
endmodule
module top;
  import p::*;
  function int tf(); return 1; endfunction
  if (1) begin : g
    function int f(); return 1; endfunction
    task t(); endtask
  end
  for (genvar i = 0; i < 1; i++) begin : l
    function int lf(); return i; endfunction
    task lt(); endtask
  end
  m u();
endmodule
"#;

fn fn_ident(m: &xezim_core::ElaboratedModule, key: &str) -> SubroutineIdent {
    m.functions[key].ident.clone().expect(key)
}

fn task_ident(m: &xezim_core::ElaboratedModule, key: &str) -> SubroutineIdent {
    m.tasks[key].ident.clone().expect(key)
}

#[test]
fn a_definition_body_subroutine_is_its_own_canonical() {
    let m = elaborate(SRC);
    assert_eq!(fn_ident(&m, "tf"), ident("tf", "", ""));
    assert_eq!(fn_ident(&m, "u.mf"), ident("u.mf", "u", ""));
    assert_eq!(task_ident(&m, "u.mt"), ident("u.mt", "u", ""));
}

#[test]
fn the_alias_and_the_original_share_one_ident_on_the_direct_path() {
    let m = elaborate(SRC);
    let f = ident("g.f", "", "g");
    assert_eq!(fn_ident(&m, "g.f"), f);
    assert_eq!(fn_ident(&m, "f"), f);
    let t = ident("g.t", "", "g");
    assert_eq!(task_ident(&m, "g.t"), t);
    assert_eq!(task_ident(&m, "t"), t);
}

#[test]
fn the_alias_and_the_original_share_one_ident_in_an_instance() {
    let m = elaborate(SRC);
    let f = ident("u.g.f", "u", "g");
    assert_eq!(fn_ident(&m, "u.g.f"), f);
    assert_eq!(fn_ident(&m, "u.f"), f);
    let t = ident("u.g.t", "u", "g");
    assert_eq!(task_ident(&m, "u.g.t"), t);
    assert_eq!(task_ident(&m, "u.t"), t);
}

/// §27.5: inner labels compose on the instance path.
#[test]
fn nested_labels_compose_in_an_instance() {
    let m = elaborate(SRC);
    let h = ident("u.g.n.h", "u", "g.n");
    assert_eq!(fn_ident(&m, "u.h"), h);
    assert_eq!(fn_ident(&m, "u.n.h"), h);
    assert_eq!(fn_ident(&m, "u.g.h"), h);
    assert_eq!(fn_ident(&m, "u.g.n.h"), h);
}

#[test]
fn a_generate_for_subroutine_is_scoped_by_its_iteration() {
    let m = elaborate(SRC);
    assert_eq!(fn_ident(&m, "l[0].lf"), ident("l[0].lf", "", "l[0]"));
    assert_eq!(task_ident(&m, "l[0].lt"), ident("l[0].lt", "", "l[0]"));
}

/// §26.3: the bare hoisted entry and the qualified entry are one declaration.
#[test]
fn a_package_subroutine_has_one_ident_across_its_entries() {
    let m = elaborate(SRC);
    let f = ident("p::pf", "", "");
    assert_eq!(fn_ident(&m, "p::pf"), f);
    assert_eq!(fn_ident(&m, "pf"), f);
    let t = ident("p::pt", "", "");
    assert_eq!(task_ident(&m, "p::pt"), t);
    assert_eq!(task_ident(&m, "pt"), t);
}
