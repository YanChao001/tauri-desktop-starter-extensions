//! Bounded expression parsing and numerical methods. No source is executed as code.

const MAX_SOURCE_BYTES: usize = 4_096;
const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 2_048;
const MAX_ARGUMENTS: usize = 16;
const ROOT_SCAN_STEPS: usize = 256;
const ROOT_ITERATIONS: usize = 128;
const INTEGRATION_EVALUATIONS: usize = 32_769;
const INTEGRATION_DEPTH: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Angle {
    Deg,
    Rad,
}

#[derive(Clone, Copy, Debug)]
pub struct Vars {
    pub x: f64,
    pub ans: f64,
    pub mem: f64,
    pub angle: Angle,
}

impl Default for Vars {
    fn default() -> Self {
        Self {
            x: 0.0,
            ans: 0.0,
            mem: 0.0,
            angle: Angle::Rad,
        }
    }
}

impl Vars {
    fn validate(self) -> Result<(), String> {
        if [self.x, self.ans, self.mem]
            .iter()
            .all(|value| value.is_finite())
        {
            Ok(())
        } else {
            Err("Variables must be finite numbers".into())
        }
    }

    fn radians(self, value: f64) -> f64 {
        match self.angle {
            Angle::Deg => value.rem_euclid(360.0).to_radians(),
            Angle::Rad => value,
        }
    }

    fn angle_value(self, radians: f64) -> f64 {
        match self.angle {
            Angle::Deg => radians.to_degrees(),
            Angle::Rad => radians,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Expression {
    node: Node,
}

impl Expression {
    pub fn eval(&self, vars: Vars) -> Result<f64, String> {
        vars.validate()?;
        self.node.eval(vars)
    }
}

pub fn evaluate(source: &str, vars: Vars) -> Result<f64, String> {
    compile(source)?.eval(vars)
}

pub fn compile(source: &str) -> Result<Expression, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Expression exceeds 4096 bytes".into());
    }
    if source.trim().is_empty() {
        return Err("Expression is empty".into());
    }
    let mut parser = Parser::new(source)?;
    let node = parser.expression(0)?;
    if parser.current.kind != Token::End {
        return Err(format!(
            "Unexpected token at byte {}; use '*' for multiplication",
            parser.current.offset
        ));
    }
    Ok(Expression { node })
}

#[derive(Clone, Debug)]
struct Node {
    kind: NodeKind,
    depth: usize,
}

#[derive(Clone, Debug)]
enum NodeKind {
    Number(f64),
    Variable(Variable),
    Negate(Box<Node>),
    Binary(Binary, Box<Node>, Box<Node>),
    Factorial(Box<Node>),
    Function(Function, Vec<Node>),
}

#[derive(Clone, Copy, Debug)]
enum Variable {
    X,
    Ans,
    Mem,
}

#[derive(Clone, Copy, Debug)]
enum Binary {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    Power,
}

#[derive(Clone, Copy, Debug)]
enum Function {
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Sinh,
    Cosh,
    Tanh,
    Sqrt,
    Cbrt,
    Abs,
    Exp,
    Ln,
    Log,
    Log2,
    Log10,
    Floor,
    Ceil,
    Round,
    Trunc,
    Sign,
    Min,
    Max,
    Pow,
    Hypot,
    Clamp,
    Deg,
    Rad,
}

impl Function {
    fn named(name: &str) -> Result<Self, String> {
        Ok(match name {
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "asin" => Self::Asin,
            "acos" => Self::Acos,
            "atan" => Self::Atan,
            "atan2" => Self::Atan2,
            "sinh" => Self::Sinh,
            "cosh" => Self::Cosh,
            "tanh" => Self::Tanh,
            "sqrt" => Self::Sqrt,
            "cbrt" => Self::Cbrt,
            "abs" => Self::Abs,
            "exp" => Self::Exp,
            "ln" => Self::Ln,
            "log" => Self::Log,
            "log2" => Self::Log2,
            "log10" => Self::Log10,
            "floor" => Self::Floor,
            "ceil" => Self::Ceil,
            "round" => Self::Round,
            "trunc" => Self::Trunc,
            "sign" => Self::Sign,
            "min" => Self::Min,
            "max" => Self::Max,
            "pow" => Self::Pow,
            "hypot" => Self::Hypot,
            "clamp" => Self::Clamp,
            "deg" => Self::Deg,
            "rad" => Self::Rad,
            _ => return Err(format!("Unknown function: {name}")),
        })
    }

    fn check_arity(self, name: &str, count: usize) -> Result<(), String> {
        let (min, max) = match self {
            Self::Atan2 | Self::Pow | Self::Hypot => (2, 2),
            Self::Clamp => (3, 3),
            Self::Min | Self::Max => (2, MAX_ARGUMENTS),
            Self::Log => (1, 2),
            _ => (1, 1),
        };
        if (min..=max).contains(&count) {
            Ok(())
        } else if min == max {
            Err(format!("Function '{name}' requires {min} argument(s)"))
        } else {
            Err(format!(
                "Function '{name}' requires between {min} and {max} arguments"
            ))
        }
    }

    fn eval(self, args: &[f64], vars: Vars) -> Result<f64, String> {
        let first = args[0];
        finite(match self {
            Self::Sin => vars.radians(first).sin(),
            Self::Cos => vars.radians(first).cos(),
            Self::Tan => {
                let radians = vars.radians(first);
                if radians.cos().abs() < 1e-14 {
                    return Err("tan is undefined at this angle".into());
                }
                radians.tan()
            }
            Self::Asin | Self::Acos => {
                if !(-1.0..=1.0).contains(&first) {
                    return Err("Inverse sine/cosine requires an argument between -1 and 1".into());
                }
                vars.angle_value(if matches!(self, Self::Asin) {
                    first.asin()
                } else {
                    first.acos()
                })
            }
            Self::Atan => vars.angle_value(first.atan()),
            Self::Atan2 => {
                if first == 0.0 && args[1] == 0.0 {
                    return Err("atan2 is undefined when both arguments are zero".into());
                }
                vars.angle_value(first.atan2(args[1]))
            }
            Self::Sinh => first.sinh(),
            Self::Cosh => first.cosh(),
            Self::Tanh => first.tanh(),
            Self::Sqrt => {
                if first < 0.0 {
                    return Err("sqrt requires a non-negative argument".into());
                }
                first.sqrt()
            }
            Self::Cbrt => first.cbrt(),
            Self::Abs => first.abs(),
            Self::Exp => first.exp(),
            Self::Ln | Self::Log | Self::Log2 | Self::Log10 => {
                if first <= 0.0 {
                    return Err("Logarithms require a positive argument".into());
                }
                match self {
                    Self::Ln => first.ln(),
                    Self::Log2 => first.log2(),
                    Self::Log if args.len() == 2 => {
                        if args[1] <= 0.0 || args[1] == 1.0 {
                            return Err(
                                "Logarithm base must be positive and different from 1".into()
                            );
                        }
                        first.log(args[1])
                    }
                    _ => first.log10(),
                }
            }
            Self::Floor => first.floor(),
            Self::Ceil => first.ceil(),
            Self::Round => first.round(),
            Self::Trunc => first.trunc(),
            Self::Sign => {
                if first == 0.0 {
                    0.0
                } else {
                    first.signum()
                }
            }
            Self::Min => args.iter().copied().fold(f64::INFINITY, f64::min),
            Self::Max => args.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            Self::Pow => power(first, args[1])?,
            Self::Hypot => first.hypot(args[1]),
            Self::Clamp => {
                if args[1] > args[2] {
                    return Err("clamp lower bound must not exceed its upper bound".into());
                }
                first.clamp(args[1], args[2])
            }
            Self::Deg => first.to_degrees(),
            Self::Rad => first.to_radians(),
        })
    }
}

impl Node {
    fn eval(&self, vars: Vars) -> Result<f64, String> {
        finite(match &self.kind {
            NodeKind::Number(value) => *value,
            NodeKind::Variable(variable) => match variable {
                Variable::X => vars.x,
                Variable::Ans => vars.ans,
                Variable::Mem => vars.mem,
            },
            NodeKind::Negate(node) => -node.eval(vars)?,
            NodeKind::Binary(operator, left, right) => {
                let left = left.eval(vars)?;
                let right = right.eval(vars)?;
                match operator {
                    Binary::Add => left + right,
                    Binary::Subtract => left - right,
                    Binary::Multiply => left * right,
                    Binary::Divide => {
                        if right == 0.0 {
                            return Err("Division by zero".into());
                        }
                        left / right
                    }
                    Binary::Modulo => {
                        if right == 0.0 {
                            return Err("Modulo by zero".into());
                        }
                        left % right
                    }
                    Binary::Power => power(left, right)?,
                }
            }
            NodeKind::Factorial(node) => {
                let value = node.eval(vars)?;
                if !(0.0..=170.0).contains(&value) || value.fract() != 0.0 {
                    return Err("Factorial requires an integer between 0 and 170".into());
                }
                (2..=value as u32).map(f64::from).product()
            }
            NodeKind::Function(function, nodes) => {
                let mut args = [0.0; MAX_ARGUMENTS];
                for (node, value) in nodes.iter().zip(args.iter_mut()) {
                    *value = node.eval(vars)?;
                }
                function.eval(&args[..nodes.len()], vars)?
            }
        })
    }
}

fn finite(value: f64) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err("Result is not finite; check the domain and numeric range".into())
    }
}

fn power(base: f64, exponent: f64) -> Result<f64, String> {
    if base == 0.0 && exponent < 0.0 {
        return Err("Zero cannot be raised to a negative power".into());
    }
    if base < 0.0 && exponent.fract() != 0.0 {
        return Err(
            "A negative base requires an integer exponent; use cbrt for real cube roots".into(),
        );
    }
    finite(base.powf(exponent))
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Bang,
    Open,
    Close,
    Comma,
    End,
}

struct PositionedToken {
    kind: Token,
    offset: usize,
}

struct Lexer<'a> {
    source: &'a str,
    position: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Result<PositionedToken, String> {
        while let Some(character) = self.source[self.position..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            self.position += character.len_utf8();
        }
        let offset = self.position;
        let Some(character) = self.source[offset..].chars().next() else {
            return Ok(PositionedToken {
                kind: Token::End,
                offset,
            });
        };
        self.position += character.len_utf8();
        let kind = match character {
            '+' => Token::Plus,
            '-' | '−' => Token::Minus,
            '*' | '×' => Token::Star,
            '/' | '÷' => Token::Slash,
            '%' => Token::Percent,
            '^' => Token::Caret,
            '!' => Token::Bang,
            '(' => Token::Open,
            ')' => Token::Close,
            ',' => Token::Comma,
            'π' => Token::Name("pi".into()),
            'τ' => Token::Name("tau".into()),
            value if value.is_ascii_digit() || value == '.' => {
                self.position = offset;
                self.number()?
            }
            value if value.is_ascii_alphabetic() || value == '_' => {
                while self
                    .source
                    .as_bytes()
                    .get(self.position)
                    .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
                {
                    self.position += 1;
                }
                Token::Name(self.source[offset..self.position].to_ascii_lowercase())
            }
            _ => return Err(format!("Unexpected character at byte {offset}")),
        };
        Ok(PositionedToken { kind, offset })
    }

    fn number(&mut self) -> Result<Token, String> {
        let start = self.position;
        let mut digits = 0;
        while self
            .source
            .as_bytes()
            .get(self.position)
            .is_some_and(u8::is_ascii_digit)
        {
            self.position += 1;
            digits += 1;
        }
        if self.source.as_bytes().get(self.position) == Some(&b'.') {
            self.position += 1;
            while self
                .source
                .as_bytes()
                .get(self.position)
                .is_some_and(u8::is_ascii_digit)
            {
                self.position += 1;
                digits += 1;
            }
        }
        if digits == 0 {
            return Err(format!("Invalid number at byte {start}"));
        }
        if matches!(self.source.as_bytes().get(self.position), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.source.as_bytes().get(self.position), Some(b'+' | b'-')) {
                self.position += 1;
            }
            let exponent_start = self.position;
            while self
                .source
                .as_bytes()
                .get(self.position)
                .is_some_and(u8::is_ascii_digit)
            {
                self.position += 1;
            }
            if self.position == exponent_start {
                return Err(format!("Invalid scientific notation at byte {start}"));
            }
        }
        let value = self.source[start..self.position]
            .parse::<f64>()
            .map_err(|_| format!("Invalid number at byte {start}"))?;
        finite(value).map(Token::Number)
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    current: PositionedToken,
    nodes: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, String> {
        let mut lexer = Lexer {
            source,
            position: 0,
        };
        let current = lexer.next()?;
        Ok(Self {
            lexer,
            current,
            nodes: 0,
        })
    }

    fn advance(&mut self) -> Result<Token, String> {
        let next = self.lexer.next()?;
        Ok(std::mem::replace(&mut self.current, next).kind)
    }

    fn node(&mut self, kind: NodeKind, depth: usize) -> Result<Node, String> {
        self.nodes += 1;
        if self.nodes > MAX_NODES {
            return Err("Expression exceeds 2048 nodes".into());
        }
        if depth > MAX_DEPTH {
            return Err("Expression exceeds nesting depth 64".into());
        }
        Ok(Node { kind, depth })
    }

    fn binary(&mut self, operator: Binary, left: Node, right: Node) -> Result<Node, String> {
        let depth = 1 + left.depth.max(right.depth);
        self.node(
            NodeKind::Binary(operator, Box::new(left), Box::new(right)),
            depth,
        )
    }

    fn expression(&mut self, depth: usize) -> Result<Node, String> {
        let mut left = self.product(depth)?;
        loop {
            let operator = match self.current.kind {
                Token::Plus => Binary::Add,
                Token::Minus => Binary::Subtract,
                _ => return Ok(left),
            };
            self.advance()?;
            let right = self.product(depth)?;
            left = self.binary(operator, left, right)?;
        }
    }

    fn product(&mut self, depth: usize) -> Result<Node, String> {
        let mut left = self.unary(depth)?;
        loop {
            let operator = match self.current.kind {
                Token::Star => Binary::Multiply,
                Token::Slash => Binary::Divide,
                Token::Percent => Binary::Modulo,
                _ => return Ok(left),
            };
            self.advance()?;
            let right = self.unary(depth)?;
            left = self.binary(operator, left, right)?;
        }
    }

    fn unary(&mut self, depth: usize) -> Result<Node, String> {
        if depth > MAX_DEPTH {
            return Err("Expression exceeds nesting depth 64".into());
        }
        match self.current.kind {
            Token::Plus => {
                self.advance()?;
                self.unary(depth + 1)
            }
            Token::Minus => {
                self.advance()?;
                let child = self.unary(depth + 1)?;
                let child_depth = child.depth;
                self.node(NodeKind::Negate(Box::new(child)), child_depth + 1)
            }
            _ => self.power(depth),
        }
    }

    fn power(&mut self, depth: usize) -> Result<Node, String> {
        let left = self.postfix(depth)?;
        if self.current.kind == Token::Caret {
            self.advance()?;
            let right = self.unary(depth + 1)?;
            self.binary(Binary::Power, left, right)
        } else {
            Ok(left)
        }
    }

    fn postfix(&mut self, depth: usize) -> Result<Node, String> {
        let mut child = self.primary(depth)?;
        while self.current.kind == Token::Bang {
            self.advance()?;
            let child_depth = child.depth;
            child = self.node(NodeKind::Factorial(Box::new(child)), child_depth + 1)?;
        }
        Ok(child)
    }

    fn primary(&mut self, depth: usize) -> Result<Node, String> {
        let offset = self.current.offset;
        match self.advance()? {
            Token::Number(value) => self.node(NodeKind::Number(value), 1),
            Token::Name(name) if self.current.kind == Token::Open => {
                let function = Function::named(&name)?;
                self.advance()?;
                let mut args = Vec::new();
                if self.current.kind != Token::Close {
                    loop {
                        if args.len() == MAX_ARGUMENTS {
                            return Err("A function accepts at most 16 arguments".into());
                        }
                        args.push(self.expression(depth + 1)?);
                        if self.current.kind != Token::Comma {
                            break;
                        }
                        self.advance()?;
                    }
                }
                self.close()?;
                function.check_arity(&name, args.len())?;
                let child_depth = args.iter().map(|node| node.depth).max().unwrap_or(0);
                self.node(NodeKind::Function(function, args), child_depth + 1)
            }
            Token::Name(name) => {
                let kind = match name.as_str() {
                    "pi" => NodeKind::Number(std::f64::consts::PI),
                    "tau" => NodeKind::Number(std::f64::consts::TAU),
                    "e" => NodeKind::Number(std::f64::consts::E),
                    "x" => NodeKind::Variable(Variable::X),
                    "ans" => NodeKind::Variable(Variable::Ans),
                    "mem" => NodeKind::Variable(Variable::Mem),
                    _ if Function::named(&name).is_ok() => {
                        return Err(format!("Function '{name}' requires parentheses"));
                    }
                    _ => return Err(format!("Unknown identifier: {name}")),
                };
                self.node(kind, 1)
            }
            Token::Open => {
                let child = self.expression(depth + 1)?;
                self.close()?;
                Ok(child)
            }
            _ => Err(format!(
                "Expected a number, variable or '(' at byte {offset}"
            )),
        }
    }

    fn close(&mut self) -> Result<(), String> {
        if self.current.kind != Token::Close {
            return Err(format!("Expected ')' at byte {}", self.current.offset));
        }
        self.advance()?;
        Ok(())
    }
}

fn checked_interval(a: f64, b: f64) -> Result<(), String> {
    if !a.is_finite() || !b.is_finite() || !(b - a).is_finite() {
        Err("Interval bounds and width must be finite".into())
    } else {
        Ok(())
    }
}

fn opposite(left: f64, right: f64) -> bool {
    left.is_sign_negative() != right.is_sign_negative()
}

/// Finds the first sampled zero or sign-changing bracket from left to right.
/// A missing bracket is an error: even-multiplicity roots are not guaranteed.
pub fn root(source: &str, a: f64, b: f64, vars: Vars) -> Result<f64, String> {
    checked_interval(a, b)?;
    vars.validate()?;
    let expression = compile(source)?;
    let (a, b) = (a.min(b), a.max(b));
    let value_at = |x| expression.eval(Vars { x, ..vars });
    let mut left = a;
    let mut left_value = value_at(left)?;
    if left_value == 0.0 {
        return Ok(left);
    }
    for step in 1..=ROOT_SCAN_STEPS {
        let right = if step == ROOT_SCAN_STEPS {
            b
        } else {
            a + (b - a) * (step as f64 / ROOT_SCAN_STEPS as f64)
        };
        let right_value = value_at(right)?;
        if right_value == 0.0 {
            return Ok(right);
        }
        if opposite(left_value, right_value) {
            return bisect(&expression, left, right, left_value, right_value, vars);
        }
        left = right;
        left_value = right_value;
    }
    Err("No sampled zero or sign-changing root found in the interval".into())
}

fn bisect(
    expression: &Expression,
    mut left: f64,
    mut right: f64,
    mut left_value: f64,
    right_value: f64,
    vars: Vars,
) -> Result<f64, String> {
    let residual_scale = left_value.abs().max(right_value.abs());
    let residual_tolerance = residual_scale * 1e-8;
    for _ in 0..ROOT_ITERATIONS {
        let midpoint = left + (right - left) * 0.5;
        let value = expression.eval(Vars {
            x: midpoint,
            ..vars
        })?;
        if value == 0.0 {
            return Ok(midpoint);
        }
        let narrow = right - left <= 1e-12 * (1.0 + midpoint.abs());
        if narrow && value.abs() <= residual_tolerance {
            return Ok(midpoint);
        }
        if midpoint == left || midpoint == right {
            return Err("Root did not converge; the interval may contain a singularity".into());
        }
        if opposite(left_value, value) {
            right = midpoint;
        } else {
            left = midpoint;
            left_value = value;
        }
    }
    Err("Root iteration limit reached; narrow the interval".into())
}

/// Adaptive Simpson quadrature with explicit depth and evaluation limits.
/// Domain errors and lack of convergence are returned instead of partial values.
pub fn integrate(source: &str, a: f64, b: f64, vars: Vars) -> Result<f64, String> {
    checked_interval(a, b)?;
    vars.validate()?;
    let expression = compile(source)?;
    if a == b {
        expression.eval(Vars { x: a, ..vars })?;
        return Ok(0.0);
    }
    let sign = if a < b { 1.0 } else { -1.0 };
    let (a, b) = (a.min(b), a.max(b));
    let midpoint = a + (b - a) * 0.5;
    if midpoint == a || midpoint == b {
        return Err("Integration interval is too narrow to subdivide".into());
    }
    let mut integrator = Integrator {
        expression: &expression,
        vars,
        evaluations: 0,
    };
    let fa = integrator.at(a)?;
    let fm = integrator.at(midpoint)?;
    let fb = integrator.at(b)?;
    let whole = simpson(a, b, fa, fm, fb)?;
    let tolerance = 1e-9 * (1.0 + whole.abs());
    finite(
        sign * integrator.adaptive(
            Segment {
                a,
                b,
                fa,
                fm,
                fb,
                whole,
            },
            tolerance,
            INTEGRATION_DEPTH,
        )?,
    )
}

struct Segment {
    a: f64,
    b: f64,
    fa: f64,
    fm: f64,
    fb: f64,
    whole: f64,
}

struct Integrator<'a> {
    expression: &'a Expression,
    vars: Vars,
    evaluations: usize,
}

impl Integrator<'_> {
    fn at(&mut self, x: f64) -> Result<f64, String> {
        if self.evaluations >= INTEGRATION_EVALUATIONS {
            return Err("Integration evaluation limit reached; narrow the interval".into());
        }
        self.evaluations += 1;
        self.expression.eval(Vars { x, ..self.vars })
    }

    fn adaptive(&mut self, segment: Segment, tolerance: f64, depth: usize) -> Result<f64, String> {
        let Segment {
            a,
            b,
            fa,
            fm,
            fb,
            whole,
        } = segment;
        let midpoint = a + (b - a) * 0.5;
        let left_midpoint = a + (midpoint - a) * 0.5;
        let right_midpoint = midpoint + (b - midpoint) * 0.5;
        if left_midpoint == a || right_midpoint == b {
            return Err(
                "Integration cannot resolve this interval; narrow the numeric range".into(),
            );
        }
        let fl = self.at(left_midpoint)?;
        let fr = self.at(right_midpoint)?;
        let left = simpson(a, midpoint, fa, fl, fm)?;
        let right = simpson(midpoint, b, fm, fr, fb)?;
        let difference = finite(finite(left + right)? - whole)?;
        if difference.abs() <= 15.0 * tolerance {
            let corrected = finite(left + right + difference / 15.0)?;
            // Dyadic samples can alias familiar periodic functions such as
            // sin(2*x)^2 on [0, 2*pi]. An independent three-point Gauss probe
            // must agree before accepting an apparently converged segment.
            let offset = (b - a) * 0.5 * (3.0_f64 / 5.0).sqrt();
            let gauss_left = self.at(midpoint - offset)?;
            let gauss_right = self.at(midpoint + offset)?;
            let gauss = finite(
                (b - a)
                    * (gauss_left * (5.0 / 18.0) + fm * (4.0 / 9.0) + gauss_right * (5.0 / 18.0)),
            )?;
            if finite(corrected - gauss)?.abs() <= 10.0 * tolerance {
                return Ok(corrected);
            }
        }
        if depth == 0 {
            return Err(
                "Integration did not converge; check for singularities or narrow the interval"
                    .into(),
            );
        }
        let left_result = self.adaptive(
            Segment {
                a,
                b: midpoint,
                fa,
                fm: fl,
                fb: fm,
                whole: left,
            },
            tolerance * 0.5,
            depth - 1,
        )?;
        let right_result = self.adaptive(
            Segment {
                a: midpoint,
                b,
                fa: fm,
                fm: fr,
                fb,
                whole: right,
            },
            tolerance * 0.5,
            depth - 1,
        )?;
        finite(left_result + right_result)
    }
}

fn simpson(a: f64, b: f64, fa: f64, fm: f64, fb: f64) -> Result<f64, String> {
    // Weight before multiplying by the interval to avoid needless 4*f(x) overflow.
    finite((b - a) * (fa / 6.0 + fm * (2.0 / 3.0) + fb / 6.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(source: &str) -> f64 {
        evaluate(source, Vars::default()).unwrap()
    }

    fn near(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-8 * (1.0 + expected.abs()),
            "actual={actual}, expected={expected}"
        );
    }

    #[test]
    fn decimal_and_scientific_numbers() {
        near(value(".5 + 2. + 1e3 + 2.5E-2"), 1002.525);
    }

    #[test]
    fn precedence_and_left_associativity() {
        near(value("20 - 3 * 4 + 8 / 2 / 2"), 10.0);
        near(value("10 - 3 - 2"), 5.0);
    }

    #[test]
    fn exponent_is_right_associative() {
        near(value("2^3^2"), 512.0);
    }

    #[test]
    fn exponent_and_unary_precedence() {
        near(value("-2^2"), -4.0);
        near(value("(-2)^2"), 4.0);
        near(value("2^-2"), 0.25);
        near(value("-2^-2"), -0.25);
        near(value("2^--2"), 4.0);
    }

    #[test]
    fn parentheses_and_unary_signs() {
        near(value("-(2 + 3) * +4"), -20.0);
        near(value("--5"), 5.0);
    }

    #[test]
    fn unicode_operator_aliases() {
        near(value("2 × π ÷ π − 1"), 1.0);
        near(value("τ / pi"), 2.0);
    }

    #[test]
    fn constants_and_case_insensitive_functions() {
        near(value("Ln(E) + COS(PI) + tau/(2*pi)"), 1.0);
    }

    #[test]
    fn variables_and_compiled_reuse() {
        let expression = compile("x^2 + ans - mem").unwrap();
        let vars = Vars {
            x: 3.0,
            ans: 5.0,
            mem: 2.0,
            ..Vars::default()
        };
        near(expression.eval(vars).unwrap(), 12.0);
        near(expression.eval(Vars { x: 4.0, ..vars }).unwrap(), 19.0);
    }

    #[test]
    fn factorial_and_its_precedence() {
        near(value("0! + 5!"), 121.0);
        near(value("-3! + 2^3!"), 58.0);
        assert!(value("170!").is_finite());
    }

    #[test]
    fn factorial_rejects_invalid_domains() {
        for source in ["(-1)!", "2.5!", "171!"] {
            assert!(evaluate(source, Vars::default()).is_err(), "{source}");
        }
    }

    #[test]
    fn radians_trigonometry() {
        near(value("sin(pi/2) + cos(0) + tan(pi/4)"), 3.0);
        near(
            value("asin(1) + acos(0) + atan(1)"),
            std::f64::consts::PI * 1.25,
        );
    }

    #[test]
    fn degrees_trigonometry() {
        let vars = Vars {
            angle: Angle::Deg,
            ..Vars::default()
        };
        near(evaluate("sin(30) + cos(60) + tan(45)", vars).unwrap(), 2.0);
        near(evaluate("asin(1)+acos(0)+atan(1)", vars).unwrap(), 225.0);
        near(evaluate("sin(-90) + sin(450)", vars).unwrap(), 0.0);
        assert!(evaluate("tan(90)", vars).is_err());
    }

    #[test]
    fn atan2_respects_quadrants_and_angle_mode() {
        near(value("atan2(1,-1)"), std::f64::consts::PI * 0.75);
        near(
            evaluate(
                "atan2(-1,-1)",
                Vars {
                    angle: Angle::Deg,
                    ..Vars::default()
                },
            )
            .unwrap(),
            -135.0,
        );
        assert!(evaluate("atan2(0,0)", Vars::default()).is_err());
    }

    #[test]
    fn logarithms_and_exponents() {
        near(
            value("ln(exp(2))+log(100)+log10(1000)+log2(8)+log(81,3)"),
            14.0,
        );
    }

    #[test]
    fn rounding_min_max_and_clamp() {
        near(value("floor(-1.2)+ceil(1.2)+round(2.5)+trunc(-2.8)"), 1.0);
        near(value("min(5,-2,8)+max(2,9,4)+clamp(12,0,5)"), 12.0);
        near(value("sign(0)+sign(-4)+sign(3)"), 0.0);
    }

    #[test]
    fn additional_real_functions() {
        near(value("sqrt(9)+cbrt(-8)+abs(-5)+pow(2,3)+hypot(3,4)"), 19.0);
        near(value("deg(pi)+rad(180)"), 180.0 + std::f64::consts::PI);
        near(value("sinh(0)+cosh(0)+tanh(0)"), 1.0);
    }

    #[test]
    fn modulo_uses_signed_remainder() {
        near(value("5.5%2"), 1.5);
        near(value("-5%2"), -1.0);
    }

    #[test]
    fn division_and_modulo_by_zero_fail() {
        assert_eq!(
            evaluate("1/0", Vars::default()).unwrap_err(),
            "Division by zero"
        );
        assert_eq!(
            evaluate("1%0", Vars::default()).unwrap_err(),
            "Modulo by zero"
        );
    }

    #[test]
    fn function_domains_fail_clearly() {
        for source in [
            "sqrt(-1)",
            "ln(0)",
            "log(-2)",
            "log(10,1)",
            "log(10,0)",
            "asin(2)",
            "acos(-2)",
            "clamp(1,3,2)",
            "(-2)^0.5",
            "0^-1",
        ] {
            assert!(evaluate(source, Vars::default()).is_err(), "{source}");
        }
    }

    #[test]
    fn non_finite_values_never_escape() {
        for source in ["1e999", "exp(1000)", "1e308*1e308", "1e308+1e308"] {
            assert!(evaluate(source, Vars::default()).is_err(), "{source}");
        }
        assert!(
            evaluate(
                "x",
                Vars {
                    x: f64::NAN,
                    ..Vars::default()
                }
            )
            .is_err()
        );
        assert!(
            evaluate(
                "0",
                Vars {
                    mem: f64::INFINITY,
                    ..Vars::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn syntax_errors_and_implicit_multiplication_are_rejected() {
        for source in [
            "", " ", ".", "1e", "1e+", "2..3", "2 pi", "2(3)", "(2+3", "2+", "2**3", "1,2",
            "sin(1,)", "()",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }

    #[test]
    fn unknown_names_and_wrong_arity_are_rejected() {
        for source in [
            "window",
            "eval(1)",
            "sin",
            "sin()",
            "sin(1,2)",
            "pow(2)",
            "min(1)",
            "clamp(1,2)",
            "log(1,2,3)",
        ] {
            assert!(compile(source).is_err(), "{source}");
        }
    }

    #[test]
    fn source_depth_and_argument_limits_are_enforced() {
        assert!(compile(&"1".repeat(MAX_SOURCE_BYTES + 1)).is_err());
        assert!(compile(&format!("{}1{}", "(".repeat(65), ")".repeat(65))).is_err());
        assert!(compile(&format!("{}1", "-".repeat(65))).is_err());
        assert!(compile(&format!("max({})", vec!["1"; 17].join(","))).is_err());
        assert!(compile(&vec!["1"; 66].join("+")).is_err());
    }

    #[test]
    fn node_budget_is_independently_enforced() {
        // A balanced expression avoids the depth cap and remains below 4096 bytes.
        let mut branch = "1".to_owned();
        for _ in 0..10 {
            branch = format!("({branch}+{branch})");
        }
        let source = format!("--{branch}");
        assert_eq!(source.len(), 4_095);
        assert!(compile(&source).unwrap_err().contains("2048 nodes"));
    }

    #[test]
    fn root_linear_and_quadratic() {
        near(
            root("x-0.12345", -1.0, 1.0, Vars::default()).unwrap(),
            0.12345,
        );
        near(
            root("x^2-2", 0.0, 2.0, Vars::default()).unwrap(),
            2.0_f64.sqrt(),
        );
    }

    #[test]
    fn root_non_polynomial_and_reversed_bounds() {
        near(
            root("cos(x)-x", 1.0, 0.0, Vars::default()).unwrap(),
            0.739_085_133_215_160_7,
        );
    }

    #[test]
    fn root_continues_refining_large_offset_intervals() {
        let expected = 1e12 + 0.001;
        assert_eq!(
            root(
                "x-ans",
                1e12 - 0.01,
                1e12 + 0.01,
                Vars {
                    ans: expected,
                    ..Vars::default()
                }
            )
            .unwrap(),
            expected
        );
    }

    #[test]
    fn root_preserves_other_variables() {
        near(
            root(
                "x-ans+mem",
                0.0,
                5.0,
                Vars {
                    ans: 4.0,
                    mem: 1.0,
                    ..Vars::default()
                },
            )
            .unwrap(),
            3.0,
        );
    }

    #[test]
    fn root_returns_sampled_and_endpoint_roots() {
        near(root("x^2", -1.0, 1.0, Vars::default()).unwrap(), 0.0);
        near(root("x-2", 2.0, 3.0, Vars::default()).unwrap(), 2.0);
        near(root("x-3", 3.0, 3.0, Vars::default()).unwrap(), 3.0);
    }

    #[test]
    fn root_does_not_claim_unbracketed_or_tiny_constant_roots() {
        for source in ["1e-20", "x^2+1", "(x-0.12345)^2"] {
            assert!(
                root(source, -1.0, 1.0, Vars::default()).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn root_rejects_singularities() {
        assert!(root("1/x", -1.0, 1.0, Vars::default()).is_err());
        assert!(root("1/(x-0.12345)", 0.0, 1.0, Vars::default()).is_err());
        assert!(root("tan(x)", 1.0, 2.0, Vars::default()).is_err());
    }

    #[test]
    fn numerical_methods_reject_invalid_bounds() {
        for (a, b) in [(f64::NAN, 1.0), (0.0, f64::INFINITY), (-f64::MAX, f64::MAX)] {
            assert!(root("x", a, b, Vars::default()).is_err());
            assert!(integrate("x", a, b, Vars::default()).is_err());
        }
    }

    #[test]
    fn integrate_polynomial_and_constant() {
        near(
            integrate("x^3-2*x+1", 0.0, 2.0, Vars::default()).unwrap(),
            2.0,
        );
        near(integrate("5", -2.0, 3.0, Vars::default()).unwrap(), 25.0);
    }

    #[test]
    fn integrate_trigonometry_and_exponential() {
        near(
            integrate("sin(x)", 0.0, std::f64::consts::PI, Vars::default()).unwrap(),
            2.0,
        );
        near(
            integrate("exp(x)", 0.0, 1.0, Vars::default()).unwrap(),
            std::f64::consts::E - 1.0,
        );
    }

    #[test]
    fn integration_does_not_accept_periodic_dyadic_aliases() {
        near(
            integrate("sin(2*x)^2", 0.0, std::f64::consts::TAU, Vars::default()).unwrap(),
            std::f64::consts::PI,
        );
        near(
            integrate("cos(4*x)", 0.0, std::f64::consts::TAU, Vars::default()).unwrap(),
            0.0,
        );
    }

    #[test]
    fn integrate_reversed_and_zero_width_intervals() {
        near(
            integrate("x^2", 2.0, 0.0, Vars::default()).unwrap(),
            -8.0 / 3.0,
        );
        near(integrate("sin(x)", 2.0, 2.0, Vars::default()).unwrap(), 0.0);
    }

    #[test]
    fn integrate_preserves_degree_mode_and_variables() {
        near(
            integrate(
                "sin(x)+mem",
                0.0,
                180.0,
                Vars {
                    mem: 1.0,
                    angle: Angle::Deg,
                    ..Vars::default()
                },
            )
            .unwrap(),
            180.0 + 360.0 / std::f64::consts::PI,
        );
    }

    #[test]
    fn integration_rejects_domain_errors_and_singularities() {
        for (source, a, b) in [
            ("1/x", -1.0, 1.0),
            ("sqrt(x)", -1.0, 1.0),
            ("ln(x)", 0.0, 1.0),
            ("1/(x-0.12345)", 0.0, 1.0),
        ] {
            assert!(
                integrate(source, a, b, Vars::default()).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn integration_limit_never_returns_a_partial_result() {
        let expression = compile("x").unwrap();
        let mut integrator = Integrator {
            expression: &expression,
            vars: Vars::default(),
            evaluations: INTEGRATION_EVALUATIONS,
        };
        assert!(integrator.at(1.0).unwrap_err().contains("evaluation limit"));
    }
}
