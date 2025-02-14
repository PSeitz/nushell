use crate::parser::hir::path::PathMember;
use crate::parser::hir::syntax_shape::{
    color_fallible_syntax, color_fallible_syntax_with, expand_atom, expand_expr, expand_syntax,
    parse_single_node, AnyExpressionShape, AtomicToken, BareShape, ExpandContext, ExpandExpression,
    ExpandSyntax, ExpansionRule, FallibleColorSyntax, FlatShape, ParseError, Peeked, SkipSyntax,
    StringShape, TestSyntax, WhitespaceShape,
};
use crate::parser::{hir, hir::Expression, hir::TokensIterator, Operator, RawNumber, RawToken};
use crate::prelude::*;
use serde::Serialize;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Copy, Clone)]
pub struct VariablePathShape;

impl ExpandExpression for VariablePathShape {
    fn name(&self) -> &'static str {
        "variable path"
    }

    fn expand_expr<'a, 'b>(
        &self,
        token_nodes: &mut TokensIterator<'_>,
        context: &ExpandContext,
    ) -> Result<hir::Expression, ParseError> {
        // 1. let the head be the first token, expecting a variable
        // 2. let the tail be an empty list of members
        // 2. while the next token (excluding ws) is a dot:
        //   1. consume the dot
        //   2. consume the next token as a member and push it onto tail

        let head = expand_expr(&VariableShape, token_nodes, context)?;
        let start = head.span;
        let mut end = start;
        let mut tail: Vec<PathMember> = vec![];

        loop {
            match DotShape.skip(token_nodes, context) {
                Err(_) => break,
                Ok(_) => {}
            }

            let member = expand_syntax(&MemberShape, token_nodes, context)?;
            let member = member.to_path_member(context.source);

            end = member.span;
            tail.push(member);
        }

        Ok(hir::Expression::path(head, tail, start.until(end)))
    }
}

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for VariablePathShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        token_nodes.atomic(|token_nodes| {
            // If the head of the token stream is not a variable, fail
            color_fallible_syntax(&VariableShape, token_nodes, context, shapes)?;

            loop {
                // look for a dot at the head of a stream
                let dot = color_fallible_syntax_with(
                    &ColorableDotShape,
                    &FlatShape::Dot,
                    token_nodes,
                    context,
                    shapes,
                );

                // if there's no dot, we're done
                match dot {
                    Err(_) => break,
                    Ok(_) => {}
                }

                // otherwise, look for a member, and if you don't find one, fail
                color_fallible_syntax(&MemberShape, token_nodes, context, shapes)?;
            }

            Ok(())
        })
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for VariablePathShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "VariablePathShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        token_nodes.atomic(|token_nodes| {
            // If the head of the token stream is not a variable, fail
            color_fallible_syntax(&VariableShape, token_nodes, context)?;

            loop {
                // look for a dot at the head of a stream
                let dot = color_fallible_syntax_with(
                    &ColorableDotShape,
                    &FlatShape::Dot,
                    token_nodes,
                    context,
                );

                // if there's no dot, we're done
                match dot {
                    Err(_) => break,
                    Ok(_) => {}
                }

                // otherwise, look for a member, and if you don't find one, fail
                color_fallible_syntax(&MemberShape, token_nodes, context)?;
            }

            Ok(())
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PathTailShape;

#[cfg(not(coloring_in_tokens))]
/// The failure mode of `PathTailShape` is a dot followed by a non-member
impl FallibleColorSyntax for PathTailShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        token_nodes.atomic(|token_nodes| loop {
            let result = color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                token_nodes,
                context,
                shapes,
            );

            match result {
                Err(_) => return Ok(()),
                Ok(_) => {}
            }

            // If we've seen a dot but not a member, fail
            color_fallible_syntax(&MemberShape, token_nodes, context, shapes)?;
        })
    }
}

#[cfg(coloring_in_tokens)]
/// The failure mode of `PathTailShape` is a dot followed by a non-member
impl FallibleColorSyntax for PathTailShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "PathTailShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        token_nodes.atomic(|token_nodes| loop {
            let result = color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                token_nodes,
                context,
            );

            match result {
                Err(_) => return Ok(()),
                Ok(_) => {}
            }

            // If we've seen a dot but not a member, fail
            color_fallible_syntax(&MemberShape, token_nodes, context)?;
        })
    }
}

impl FormatDebug for Spanned<Vec<PathMember>> {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        f.say_list(
            "path tail",
            &self.item,
            |f| write!(f, "["),
            |f, item| write!(f, "{}", item.debug(source)),
            |f| write!(f, " "),
            |f| write!(f, "]"),
        )
    }
}

impl ExpandSyntax for PathTailShape {
    type Output = Spanned<Vec<PathMember>>;

    fn name(&self) -> &'static str {
        "path continuation"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        let mut end: Option<Span> = None;
        let mut tail: Vec<PathMember> = vec![];

        loop {
            match DotShape.skip(token_nodes, context) {
                Err(_) => break,
                Ok(_) => {}
            }

            let member = expand_syntax(&MemberShape, token_nodes, context)?;
            let member = member.to_path_member(context.source);
            end = Some(member.span);
            tail.push(member);
        }

        match end {
            None => Err(ParseError::mismatch(
                "path tail",
                token_nodes.typed_span_at_cursor(),
            )),

            Some(end) => Ok(tail.spanned(end)),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExpressionContinuation {
    DotSuffix(Span, PathMember),
    InfixSuffix(Spanned<Operator>, Expression),
}

impl FormatDebug for ExpressionContinuation {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        match self {
            ExpressionContinuation::DotSuffix(dot, rest) => {
                f.say_str("dot suffix", dot.until(rest.span).slice(source))
            }
            ExpressionContinuation::InfixSuffix(operator, expr) => {
                f.say_str("infix suffix", operator.span.until(expr.span).slice(source))
            }
        }
    }
}

impl HasSpan for ExpressionContinuation {
    fn span(&self) -> Span {
        match self {
            ExpressionContinuation::DotSuffix(dot, column) => dot.until(column.span),
            ExpressionContinuation::InfixSuffix(operator, expression) => {
                operator.span.until(expression.span)
            }
        }
    }
}

/// An expression continuation
#[derive(Debug, Copy, Clone)]
pub struct ExpressionContinuationShape;

impl ExpandSyntax for ExpressionContinuationShape {
    type Output = ExpressionContinuation;

    fn name(&self) -> &'static str {
        "expression continuation"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &mut TokensIterator<'_>,
        context: &ExpandContext,
    ) -> Result<ExpressionContinuation, ParseError> {
        // Try to expand a `.`
        let dot = expand_syntax(&DotShape, token_nodes, context);

        match dot {
            // If a `.` was matched, it's a `Path`, and we expect a `Member` next
            Ok(dot) => {
                let syntax = expand_syntax(&MemberShape, token_nodes, context)?;
                let member = syntax.to_path_member(context.source);

                Ok(ExpressionContinuation::DotSuffix(dot, member))
            }

            // Otherwise, we expect an infix operator and an expression next
            Err(_) => {
                let (_, op, _) = expand_syntax(&InfixShape, token_nodes, context)?.item;
                let next = expand_expr(&AnyExpressionShape, token_nodes, context)?;

                Ok(ExpressionContinuation::InfixSuffix(op, next))
            }
        }
    }
}

pub enum ContinuationInfo {
    Dot,
    Infix,
}

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for ExpressionContinuationShape {
    type Info = ContinuationInfo;
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<ContinuationInfo, ShellError> {
        token_nodes.atomic(|token_nodes| {
            // Try to expand a `.`
            let dot = color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                token_nodes,
                context,
                shapes,
            );

            match dot {
                Ok(_) => {
                    // we found a dot, so let's keep looking for a member; if no member was found, fail
                    color_fallible_syntax(&MemberShape, token_nodes, context, shapes)?;

                    Ok(ContinuationInfo::Dot)
                }
                Err(_) => {
                    let mut new_shapes = vec![];
                    let result = token_nodes.atomic(|token_nodes| {
                        // we didn't find a dot, so let's see if we're looking at an infix. If not found, fail
                        color_fallible_syntax(&InfixShape, token_nodes, context, &mut new_shapes)?;

                        // now that we've seen an infix shape, look for any expression. If not found, fail
                        color_fallible_syntax(
                            &AnyExpressionShape,
                            token_nodes,
                            context,
                            &mut new_shapes,
                        )?;

                        Ok(ContinuationInfo::Infix)
                    })?;
                    shapes.extend(new_shapes);
                    Ok(result)
                }
            }
        })
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for ExpressionContinuationShape {
    type Info = ContinuationInfo;
    type Input = ();

    fn name(&self) -> &'static str {
        "ExpressionContinuationShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<ContinuationInfo, ShellError> {
        token_nodes.atomic(|token_nodes| {
            // Try to expand a `.`
            let dot = color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                token_nodes,
                context,
            );

            match dot {
                Ok(_) => {
                    // we found a dot, so let's keep looking for a member; if no member was found, fail
                    color_fallible_syntax(&MemberShape, token_nodes, context)?;

                    Ok(ContinuationInfo::Dot)
                }
                Err(_) => {
                    let result = token_nodes.atomic(|token_nodes| {
                        // we didn't find a dot, so let's see if we're looking at an infix. If not found, fail
                        color_fallible_syntax(&InfixShape, token_nodes, context)?;

                        // now that we've seen an infix shape, look for any expression. If not found, fail
                        color_fallible_syntax(&AnyExpressionShape, token_nodes, context)?;

                        Ok(ContinuationInfo::Infix)
                    })?;

                    Ok(result)
                }
            }
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct VariableShape;

impl ExpandExpression for VariableShape {
    fn name(&self) -> &'static str {
        "variable"
    }

    fn expand_expr<'a, 'b>(
        &self,
        token_nodes: &mut TokensIterator<'_>,
        context: &ExpandContext,
    ) -> Result<hir::Expression, ParseError> {
        parse_single_node(token_nodes, "variable", |token, token_tag, err| {
            Ok(match token {
                RawToken::Variable(tag) => {
                    if tag.slice(context.source) == "it" {
                        hir::Expression::it_variable(tag, token_tag)
                    } else {
                        hir::Expression::variable(tag, token_tag)
                    }
                }
                _ => return Err(err.error()),
            })
        })
    }
}

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for VariableShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        let atom = expand_atom(
            token_nodes,
            "variable",
            context,
            ExpansionRule::permissive(),
        );

        let atom = match atom {
            Err(err) => return Err(err.into()),
            Ok(atom) => atom,
        };

        match &atom.item {
            AtomicToken::Variable { .. } => {
                shapes.push(FlatShape::Variable.spanned(atom.span));
                Ok(())
            }
            AtomicToken::ItVariable { .. } => {
                shapes.push(FlatShape::ItVariable.spanned(atom.span));
                Ok(())
            }
            _ => Err(ShellError::type_error("variable", atom.spanned_type_name())),
        }
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for VariableShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "VariableShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        let atom = expand_atom(
            token_nodes,
            "variable",
            context,
            ExpansionRule::permissive(),
        );

        let atom = match atom {
            Err(err) => return Err(err.into()),
            Ok(atom) => atom,
        };

        match &atom.item {
            AtomicToken::Variable { .. } => {
                token_nodes.color_shape(FlatShape::Variable.spanned(atom.span));
                Ok(())
            }
            AtomicToken::ItVariable { .. } => {
                token_nodes.color_shape(FlatShape::ItVariable.spanned(atom.span));
                Ok(())
            }
            _ => Err(ParseError::mismatch("variable", atom.type_name().spanned(atom.span)).into()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Member {
    String(/* outer */ Span, /* inner */ Span),
    Int(BigInt, Span),
    Bare(Span),
}

impl ShellTypeName for Member {
    fn type_name(&self) -> &'static str {
        match self {
            Member::String(_, _) => "string",
            Member::Int(_, _) => "integer",
            Member::Bare(_) => "word",
        }
    }
}

impl Member {
    pub fn to_path_member(&self, source: &Text) -> PathMember {
        match self {
            Member::String(outer, inner) => PathMember::string(inner.slice(source), *outer),
            Member::Int(int, span) => PathMember::int(int.clone(), *span),
            Member::Bare(span) => PathMember::string(span.slice(source), *span),
        }
    }
}

impl FormatDebug for Member {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        match self {
            Member::String(outer, _) => write!(f, "{}", outer.slice(source)),
            Member::Int(_, int) => write!(f, "{}", int.slice(source)),
            Member::Bare(bare) => write!(f, "{}", bare.slice(source)),
        }
    }
}

impl HasSpan for Member {
    fn span(&self) -> Span {
        match self {
            Member::String(outer, ..) => *outer,
            Member::Int(_, int) => *int,
            Member::Bare(name) => *name,
        }
    }
}

impl Member {
    pub fn to_expr(&self) -> hir::Expression {
        match self {
            Member::String(outer, inner) => hir::Expression::string(*inner, *outer),
            Member::Int(number, span) => hir::Expression::number(number.clone(), *span),
            Member::Bare(span) => hir::Expression::string(*span, *span),
        }
    }

    pub(crate) fn span(&self) -> Span {
        match self {
            Member::String(outer, _inner) => *outer,
            Member::Int(_, span) => *span,
            Member::Bare(span) => *span,
        }
    }
}

enum ColumnPathState {
    Initial,
    LeadingDot(Span),
    Dot(Span, Vec<Member>, Span),
    Member(Span, Vec<Member>),
    Error(ParseError),
}

impl ColumnPathState {
    pub fn dot(self, dot: Span) -> ColumnPathState {
        match self {
            ColumnPathState::Initial => ColumnPathState::LeadingDot(dot),
            ColumnPathState::LeadingDot(_) => {
                ColumnPathState::Error(ParseError::mismatch("column", "dot".spanned(dot)))
            }
            ColumnPathState::Dot(..) => {
                ColumnPathState::Error(ParseError::mismatch("column", "dot".spanned(dot)))
            }
            ColumnPathState::Member(tag, members) => ColumnPathState::Dot(tag, members, dot),
            ColumnPathState::Error(err) => ColumnPathState::Error(err),
        }
    }

    pub fn member(self, member: Member) -> ColumnPathState {
        match self {
            ColumnPathState::Initial => ColumnPathState::Member(member.span(), vec![member]),
            ColumnPathState::LeadingDot(tag) => {
                ColumnPathState::Member(tag.until(member.span()), vec![member])
            }

            ColumnPathState::Dot(tag, mut tags, _) => {
                ColumnPathState::Member(tag.until(member.span()), {
                    tags.push(member);
                    tags
                })
            }
            ColumnPathState::Member(..) => ColumnPathState::Error(ParseError::mismatch(
                "column",
                member.type_name().spanned(member.span()),
            )),
            ColumnPathState::Error(err) => ColumnPathState::Error(err),
        }
    }

    pub fn into_path(self, next: Peeked) -> Result<Tagged<Vec<Member>>, ParseError> {
        match self {
            ColumnPathState::Initial => Err(next.type_error("column path")),
            ColumnPathState::LeadingDot(dot) => {
                Err(ParseError::mismatch("column", "dot".spanned(dot)))
            }
            ColumnPathState::Dot(_tag, _members, dot) => {
                Err(ParseError::mismatch("column", "dot".spanned(dot)))
            }
            ColumnPathState::Member(tag, tags) => Ok(tags.tagged(tag)),
            ColumnPathState::Error(err) => Err(err),
        }
    }
}

pub fn expand_column_path<'a, 'b>(
    token_nodes: &'b mut TokensIterator<'a>,
    context: &ExpandContext,
) -> Result<Tagged<Vec<Member>>, ParseError> {
    let mut state = ColumnPathState::Initial;

    loop {
        let member = expand_syntax(&MemberShape, token_nodes, context);

        match member {
            Err(_) => break,
            Ok(member) => state = state.member(member),
        }

        let dot = expand_syntax(&DotShape, token_nodes, context);

        match dot {
            Err(_) => break,
            Ok(dot) => state = state.dot(dot),
        }
    }

    state.into_path(token_nodes.peek_non_ws())
}

#[derive(Debug, Copy, Clone)]
pub struct ColumnPathShape;

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for ColumnPathShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        // If there's not even one member shape, fail
        color_fallible_syntax(&MemberShape, token_nodes, context, shapes)?;

        loop {
            let checkpoint = token_nodes.checkpoint();

            match color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                checkpoint.iterator,
                context,
                shapes,
            ) {
                Err(_) => {
                    // we already saw at least one member shape, so return successfully
                    return Ok(());
                }

                Ok(_) => {
                    match color_fallible_syntax(&MemberShape, checkpoint.iterator, context, shapes)
                    {
                        Err(_) => {
                            // we saw a dot but not a member (but we saw at least one member),
                            // so don't commit the dot but return successfully
                            return Ok(());
                        }

                        Ok(_) => {
                            // we saw a dot and a member, so commit it and continue on
                            checkpoint.commit();
                        }
                    }
                }
            }
        }
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for ColumnPathShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "ColumnPathShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        // If there's not even one member shape, fail
        color_fallible_syntax(&MemberShape, token_nodes, context)?;

        loop {
            let checkpoint = token_nodes.checkpoint();

            match color_fallible_syntax_with(
                &ColorableDotShape,
                &FlatShape::Dot,
                checkpoint.iterator,
                context,
            ) {
                Err(_) => {
                    // we already saw at least one member shape, so return successfully
                    return Ok(());
                }

                Ok(_) => {
                    match color_fallible_syntax(&MemberShape, checkpoint.iterator, context) {
                        Err(_) => {
                            // we saw a dot but not a member (but we saw at least one member),
                            // so don't commit the dot but return successfully
                            return Ok(());
                        }

                        Ok(_) => {
                            // we saw a dot and a member, so commit it and continue on
                            checkpoint.commit();
                        }
                    }
                }
            }
        }
    }
}

impl FormatDebug for Tagged<Vec<Member>> {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        self.item.fmt_debug(f, source)
    }
}

impl ExpandSyntax for ColumnPathShape {
    type Output = Tagged<Vec<Member>>;

    fn name(&self) -> &'static str {
        "column path"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        Ok(expand_column_path(token_nodes, context)?)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MemberShape;

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for MemberShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        let bare = color_fallible_syntax_with(
            &BareShape,
            &FlatShape::BareMember,
            token_nodes,
            context,
            shapes,
        );

        match bare {
            Ok(_) => return Ok(()),
            Err(_) => {
                // If we don't have a bare word, we'll look for a string
            }
        }

        // Look for a string token. If we don't find one, fail
        color_fallible_syntax_with(
            &StringShape,
            &FlatShape::StringMember,
            token_nodes,
            context,
            shapes,
        )
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for MemberShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "MemberShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        let bare =
            color_fallible_syntax_with(&BareShape, &FlatShape::BareMember, token_nodes, context);

        match bare {
            Ok(_) => return Ok(()),
            Err(_) => {
                // If we don't have a bare word, we'll look for a string
            }
        }

        // Look for a string token. If we don't find one, fail
        color_fallible_syntax_with(&StringShape, &FlatShape::StringMember, token_nodes, context)
    }
}

#[derive(Debug, Copy, Clone)]
struct IntMemberShape;

impl ExpandSyntax for IntMemberShape {
    type Output = Member;

    fn name(&self) -> &'static str {
        "integer member"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        token_nodes.atomic_parse(|token_nodes| {
            let next = expand_atom(
                token_nodes,
                "integer member",
                context,
                ExpansionRule::new().separate_members(),
            )?;

            match next.item {
                AtomicToken::Number {
                    number: RawNumber::Int(int),
                } => Ok(Member::Int(
                    BigInt::from_str(int.slice(context.source)).unwrap(),
                    int,
                )),

                AtomicToken::Word { text } => {
                    let int = BigInt::from_str(text.slice(context.source));

                    match int {
                        Ok(int) => return Ok(Member::Int(int, text)),
                        Err(_) => Err(ParseError::mismatch("integer member", "word".spanned(text))),
                    }
                }

                other => Err(ParseError::mismatch(
                    "integer member",
                    other.type_name().spanned(next.span),
                )),
            }
        })
    }
}

impl ExpandSyntax for MemberShape {
    type Output = Member;

    fn name(&self) -> &'static str {
        "column"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &mut TokensIterator<'_>,
        context: &ExpandContext,
    ) -> Result<Member, ParseError> {
        if let Ok(int) = expand_syntax(&IntMemberShape, token_nodes, context) {
            return Ok(int);
        }

        let bare = BareShape.test(token_nodes, context);
        if let Some(peeked) = bare {
            let node = peeked.not_eof("column")?.commit();
            return Ok(Member::Bare(node.span()));
        }

        /* KATZ */
        /* let number = NumberShape.test(token_nodes, context);

        if let Some(peeked) = number {
            let node = peeked.not_eof("column")?.commit();
            let (n, span) = node.as_number().unwrap();

            return Ok(Member::Number(n, span))
        }*/

        let string = StringShape.test(token_nodes, context);

        if let Some(peeked) = string {
            let node = peeked.not_eof("column")?.commit();
            let (outer, inner) = node.as_string().unwrap();

            return Ok(Member::String(outer, inner));
        }

        Err(token_nodes.peek_any().type_error("column"))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DotShape;

#[derive(Debug, Copy, Clone)]
pub struct ColorableDotShape;

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for ColorableDotShape {
    type Info = ();
    type Input = FlatShape;

    fn color_syntax<'a, 'b>(
        &self,
        input: &FlatShape,
        token_nodes: &'b mut TokensIterator<'a>,
        _context: &ExpandContext,
        shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        let peeked = token_nodes.peek_any().not_eof("dot")?;

        match peeked.node {
            node if node.is_dot() => {
                peeked.commit();
                shapes.push((*input).spanned(node.span()));
                Ok(())
            }

            other => Err(ShellError::type_error("dot", other.spanned_type_name())),
        }
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for ColorableDotShape {
    type Info = ();
    type Input = FlatShape;

    fn name(&self) -> &'static str {
        "ColorableDotShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        input: &FlatShape,
        token_nodes: &'b mut TokensIterator<'a>,
        _context: &ExpandContext,
    ) -> Result<(), ShellError> {
        let peeked = token_nodes.peek_any().not_eof("dot")?;

        match peeked.node {
            node if node.is_dot() => {
                peeked.commit();
                token_nodes.color_shape((*input).spanned(node.span()));
                Ok(())
            }

            other => Err(ShellError::type_error(
                "dot",
                other.type_name().spanned(other.span()),
            )),
        }
    }
}

impl SkipSyntax for DotShape {
    fn skip<'a, 'b>(
        &self,
        token_nodes: &mut TokensIterator<'_>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        expand_syntax(self, token_nodes, context)?;

        Ok(())
    }
}

impl ExpandSyntax for DotShape {
    type Output = Span;

    fn name(&self) -> &'static str {
        "dot"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        _context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        parse_single_node(token_nodes, "dot", |token, token_span, _| {
            Ok(match token {
                RawToken::Operator(Operator::Dot) => token_span,
                _ => {
                    return Err(ParseError::mismatch(
                        "dot",
                        token.type_name().spanned(token_span),
                    ))
                }
            })
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InfixShape;

#[cfg(not(coloring_in_tokens))]
impl FallibleColorSyntax for InfixShape {
    type Info = ();
    type Input = ();

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
        outer_shapes: &mut Vec<Spanned<FlatShape>>,
    ) -> Result<(), ShellError> {
        let checkpoint = token_nodes.checkpoint();
        let mut shapes = vec![];

        // An infix operator must be prefixed by whitespace. If no whitespace was found, fail
        color_fallible_syntax(&WhitespaceShape, checkpoint.iterator, context, &mut shapes)?;

        // Parse the next TokenNode after the whitespace
        parse_single_node(
            checkpoint.iterator,
            "infix operator",
            |token, token_span, err| {
                match token {
                    // If it's an operator (and not `.`), it's a match
                    RawToken::Operator(operator) if operator != Operator::Dot => {
                        shapes.push(FlatShape::Operator.spanned(token_span));
                        Ok(())
                    }

                    // Otherwise, it's not a match
                    _ => Err(err.error()),
                }
            },
        )?;

        // An infix operator must be followed by whitespace. If no whitespace was found, fail
        color_fallible_syntax(&WhitespaceShape, checkpoint.iterator, context, &mut shapes)?;

        outer_shapes.extend(shapes);
        checkpoint.commit();
        Ok(())
    }
}

#[cfg(coloring_in_tokens)]
impl FallibleColorSyntax for InfixShape {
    type Info = ();
    type Input = ();

    fn name(&self) -> &'static str {
        "InfixShape"
    }

    fn color_syntax<'a, 'b>(
        &self,
        _input: &(),
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<(), ShellError> {
        let checkpoint = token_nodes.checkpoint();

        // An infix operator must be prefixed by whitespace. If no whitespace was found, fail
        color_fallible_syntax(&WhitespaceShape, checkpoint.iterator, context)?;

        // Parse the next TokenNode after the whitespace
        let operator_span = parse_single_node(
            checkpoint.iterator,
            "infix operator",
            |token, token_span, _| {
                match token {
                    // If it's an operator (and not `.`), it's a match
                    RawToken::Operator(operator) if operator != Operator::Dot => Ok(token_span),

                    // Otherwise, it's not a match
                    _ => Err(ParseError::mismatch(
                        "infix operator",
                        token.type_name().spanned(token_span),
                    )),
                }
            },
        )?;

        checkpoint
            .iterator
            .color_shape(FlatShape::Operator.spanned(operator_span));

        // An infix operator must be followed by whitespace. If no whitespace was found, fail
        color_fallible_syntax(&WhitespaceShape, checkpoint.iterator, context)?;

        checkpoint.commit();
        Ok(())
    }
}

impl FormatDebug for Spanned<(Span, Spanned<Operator>, Span)> {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        f.say_str("operator", self.item.1.span.slice(source))
    }
}

impl ExpandSyntax for InfixShape {
    type Output = Spanned<(Span, Spanned<Operator>, Span)>;

    fn name(&self) -> &'static str {
        "infix operator"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        let mut checkpoint = token_nodes.checkpoint();

        // An infix operator must be prefixed by whitespace
        let start = expand_syntax(&WhitespaceShape, checkpoint.iterator, context)?;

        // Parse the next TokenNode after the whitespace
        let operator = expand_syntax(&InfixInnerShape, &mut checkpoint.iterator, context)?;

        // An infix operator must be followed by whitespace
        let end = expand_syntax(&WhitespaceShape, checkpoint.iterator, context)?;

        checkpoint.commit();

        Ok((start, operator, end).spanned(start.until(end)))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InfixInnerShape;

impl FormatDebug for Spanned<Operator> {
    fn fmt_debug(&self, f: &mut DebugFormatter, source: &str) -> fmt::Result {
        f.say_str("operator", self.span.slice(source))
    }
}

impl ExpandSyntax for InfixInnerShape {
    type Output = Spanned<Operator>;

    fn name(&self) -> &'static str {
        "infix inner"
    }

    fn expand_syntax<'a, 'b>(
        &self,
        token_nodes: &'b mut TokensIterator<'a>,
        _context: &ExpandContext,
    ) -> Result<Self::Output, ParseError> {
        parse_single_node(token_nodes, "infix operator", |token, token_span, err| {
            Ok(match token {
                // If it's an operator (and not `.`), it's a match
                RawToken::Operator(operator) if operator != Operator::Dot => {
                    operator.spanned(token_span)
                }

                // Otherwise, it's not a match
                _ => return Err(err.error()),
            })
        })
    }
}
