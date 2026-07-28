// Coverage gap (S1 Task 8 corpus sweep): ConditionalExpression,
// LogicalExpression, SequenceExpression and TemplateLiteral/TemplateElement
// have no C++ `SemanticResolver::visit` override (mod.rs's override-free
// generic-dispatch whitelist, cpp inventory in SemanticResolver.h:200-304),
// so they route through the generic `visit_children_mut` rebuild — but no
// corpus file exercised that path for these four kinds. This file does.
var a, b, c;
var cond = a ? b : c;
var log = a && b || c;
var seq = (a, b, c);
var tpl = `x=${a} y=${b}`;
