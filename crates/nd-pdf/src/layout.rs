use krilla::Document;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect};
use krilla::metadata::Metadata;
use krilla::page::PageSettings;
use krilla::paint::Fill;
use krilla::text::TextDirection;

use crate::fonts::{Face, Faces, runs};
use crate::{PdfBlock, PdfError, PdfReport, PdfStyle, PdfTone};

const W: f32 = 595.28;
const H: f32 = 841.89;
const LEFT: f32 = 56.0;
const RIGHT: f32 = W - 56.0;
const TOP: f32 = 64.0;
const BOTTOM: f32 = H - 66.0;
const WIDTH: f32 = RIGHT - LEFT;

type Rgb = (u8, u8, u8);

const INK: Rgb = (0x1c, 0x1b, 0x19);
const MUTED: Rgb = (0x6a, 0x67, 0x61);
const RULE: Rgb = (0xcf, 0xcb, 0xc3);
const CODE_BG: Rgb = (0xf3, 0xf1, 0xec);

fn color(t: PdfTone) -> Rgb {
    match t {
        PdfTone::Plain => INK,
        PdfTone::Muted => MUTED,
        PdfTone::High => (0xa8, 0x2a, 0x1f),
        PdfTone::Medium => (0x9a, 0x61, 0x0c),
        PdfTone::Low => (0x4d, 0x5f, 0x70),
        PdfTone::Ok => (0x2b, 0x6e, 0x47),
    }
}

#[derive(Clone, Copy)]
enum F {
    Body,
    Strong,
    Display,
    Mono,
}

enum Op {
    Text {
        x: f32,
        y: f32,
        face: F,
        size: f32,
        rgb: Rgb,
        text: String,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgb: Rgb,
    },
}

struct Layout<'a> {
    faces: &'a Faces,
    style: PdfStyle,
    title: String,
    pages: Vec<Vec<Op>>,
    y: f32,
}

impl Layout<'_> {
    fn face(&self, f: F) -> &Face {
        match f {
            F::Body => &self.faces.body,
            F::Strong => &self.faces.strong,
            F::Display => &self.faces.display,
            F::Mono => &self.faces.mono,
        }
    }

    fn page(&mut self) -> &mut Vec<Op> {
        if self.pages.is_empty() {
            self.pages.push(Vec::new());
        }
        let last = self.pages.len() - 1;
        &mut self.pages[last]
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = TOP;
        let title = self.title.clone();
        self.text(LEFT, 40.0, F::Body, 7.5, MUTED, &title);
        self.rect(LEFT, 46.0, WIDTH, 0.4, RULE);
    }

    fn ensure(&mut self, h: f32) {
        if self.y + h > BOTTOM {
            self.new_page();
        }
    }

    fn text(&mut self, x: f32, baseline: f32, face: F, size: f32, rgb: Rgb, text: &str) {
        let text: String = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\t')
            .collect();
        if text.trim().is_empty() {
            return;
        }
        self.page().push(Op::Text {
            x,
            y: baseline,
            face,
            size,
            rgb,
            text,
        });
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, rgb: Rgb) {
        self.page().push(Op::Rect { x, y, w, h, rgb });
    }

    fn width(&self, face: F, size: f32, text: &str) -> f32 {
        runs(self.face(face), &self.faces.fallback, text)
            .iter()
            .map(|(f, s)| f.width(s, size))
            .sum()
    }

    fn wrap(&self, face: F, size: f32, text: &str, width: f32) -> Vec<String> {
        let mut out = Vec::new();
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split(' ') {
                let candidate = if line.is_empty() {
                    word.to_owned()
                } else {
                    format!("{line} {word}")
                };
                if self.width(face, size, &candidate) <= width {
                    line = candidate;
                    continue;
                }
                if !line.is_empty() {
                    out.push(std::mem::take(&mut line));
                }
                let mut piece = String::new();
                for c in word.chars() {
                    piece.push(c);
                    if self.width(face, size, &piece) > width && piece.chars().count() > 1 {
                        piece.pop();
                        out.push(std::mem::take(&mut piece));
                        piece.push(c);
                    }
                }
                line = piece;
            }
            out.push(line);
        }
        out
    }

    fn lines(&mut self, x: f32, face: F, size: f32, leading: f32, rgb: Rgb, text: &str) {
        let width = RIGHT - x;
        for line in self.wrap(face, size, text, width) {
            self.ensure(leading);
            let baseline = self.y + leading * 0.76;
            self.text(x, baseline, face, size, rgb, &line);
            self.y += leading;
        }
    }

    fn caps(&mut self, x: f32, face: F, size: f32, rgb: Rgb, text: &str) {
        let baseline = self.y + size;
        self.text(x, baseline, face, size, rgb, &text.to_uppercase());
    }

    fn title_rule(&mut self) {
        let y = self.y;
        match self.style {
            PdfStyle::Registro => {
                self.rect(LEFT, y, WIDTH, 0.6, INK);
                self.rect(LEFT, y + 2.2, WIDTH, 0.6, INK);
                self.y += 3.0;
            }
            PdfStyle::Banco => {
                self.rect(LEFT, y, WIDTH, 2.4, INK);
                self.y += 2.4;
            }
            PdfStyle::Tavola => {
                self.rect(LEFT, y, WIDTH, 0.8, INK);
                self.y += 0.8;
            }
            PdfStyle::Listino => {
                self.rect(LEFT, y, WIDTH, 0.8, INK);
                self.rect(LEFT, y, 48.0, 2.4, color(PdfTone::Medium));
                self.y += 2.4;
            }
        }
    }

    fn head(&mut self, r: &PdfReport) {
        self.y = TOP - 8.0;
        self.lines(LEFT, F::Display, 22.0, 27.0, INK, &r.title);
        self.y += 2.0;
        self.lines(LEFT, F::Body, 9.0, 13.0, MUTED, &r.subtitle);
        self.y += 8.0;
        self.title_rule();
        self.y += 12.0;
        if let Some(lead) = &r.lead {
            self.lines(LEFT, F::Body, 9.0, 13.0, MUTED, lead);
            self.y += 10.0;
        }
        if !r.tally.is_empty() {
            self.tally(r);
        }
    }

    fn tally(&mut self, r: &PdfReport) {
        let n = r.tally.len();
        #[allow(clippy::cast_precision_loss)]
        let cell = WIDTH / n as f32;
        let top = self.y;
        self.rect(LEFT, top, WIDTH, 0.4, RULE);
        for (i, t) in r.tally.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let x = LEFT + cell * i as f32;
            if i > 0 {
                self.rect(x, top + 6.0, 0.4, 40.0, RULE);
            }
            let pad = if i == 0 { 0.0 } else { 10.0 };
            self.text(
                x + pad,
                top + 30.0,
                F::Display,
                22.0,
                color(t.tone),
                &t.value,
            );
            let label = t.label.to_uppercase();
            self.text(x + pad, top + 44.0, F::Strong, 6.8, MUTED, &label);
        }
        self.y = top + 52.0;
        self.rect(LEFT, self.y, WIDTH, 0.4, RULE);
        self.y += 18.0;
    }

    fn heading(&mut self, text: &str, count: Option<&str>, tone: PdfTone) {
        self.ensure(70.0);
        self.y += 10.0;
        let baseline = self.y + 14.0;
        self.text(LEFT, baseline, F::Display, 13.5, INK, text);
        if let Some(c) = count {
            let x = LEFT + self.width(F::Display, 13.5, text) + 8.0;
            self.text(x, baseline, F::Body, 9.0, MUTED, c);
        }
        self.y += 20.0;
        let rule = if tone == PdfTone::Plain || tone == PdfTone::Muted {
            RULE
        } else {
            color(tone)
        };
        self.rect(LEFT, self.y, WIDTH, 0.8, rule);
        self.y += 8.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn finding(
        &mut self,
        severity: &str,
        tone: PdfTone,
        category: &str,
        title: &str,
        what: &str,
        subjects: (&str, &[String]),
        fix: (&str, Option<&str>),
    ) {
        let count = |face, size, text: &str, x: f32| {
            #[allow(clippy::cast_precision_loss)]
            let n = self.wrap(face, size, text, RIGHT - x).len() as f32;
            n
        };
        let mut height = 37.0
            + count(F::Display, 11.5, title, LEFT) * 15.0
            + count(F::Body, 9.0, what, LEFT) * 13.0;
        if !subjects.1.is_empty() {
            height += 16.0
                + subjects
                    .1
                    .iter()
                    .map(|s| count(F::Mono, 8.0, s, LEFT + 10.0) * 11.5)
                    .sum::<f32>();
        }
        if let Some(sql) = fix.1 {
            height += 25.0 + count(F::Mono, 8.0, sql, LEFT + 18.0) * 11.5;
        }
        self.ensure(height.min(BOTTOM - TOP));
        self.y += 8.0;
        let sev = severity.to_uppercase();
        self.caps(LEFT, F::Strong, 7.2, color(tone), &sev);
        let cat = category.to_uppercase();
        let w = self.width(F::Body, 7.2, &cat);
        self.caps(RIGHT - w, F::Body, 7.2, MUTED, &cat);
        self.y += 12.0;
        self.lines(LEFT, F::Display, 11.5, 15.0, INK, title);
        self.y += 1.0;
        self.lines(LEFT, F::Body, 9.0, 13.0, INK, what);
        let (label, items) = subjects;
        if !items.is_empty() {
            self.y += 5.0;
            self.ensure(24.0);
            self.caps(LEFT, F::Strong, 6.6, MUTED, label);
            self.y += 11.0;
            for item in items {
                self.ensure(11.5);
                self.text(LEFT, self.y + 8.6, F::Mono, 8.0, MUTED, "–");
                self.lines(LEFT + 10.0, F::Mono, 8.0, 11.5, INK, item);
            }
        }
        let (label, sql) = fix;
        if let Some(sql) = sql {
            self.y += 6.0;
            self.ensure(30.0);
            self.caps(LEFT, F::Strong, 6.6, MUTED, label);
            self.y += 11.0;
            self.code(sql);
        }
        self.y += 8.0;
        self.rect(LEFT, self.y, WIDTH, 0.4, RULE);
    }

    fn code(&mut self, sql: &str) {
        const PAD: f32 = 4.0;
        const LEAD: f32 = 11.5;
        let lines = self.wrap(F::Mono, 8.0, sql, WIDTH - 2.0 * PAD - 8.0);
        let count = lines.len();
        for (i, line) in lines.into_iter().enumerate() {
            let first = i == 0;
            let last = i + 1 == count;
            let h = LEAD + if first { PAD } else { 0.0 } + if last { PAD } else { 0.0 };
            self.ensure(h);
            self.rect(LEFT, self.y, WIDTH, h, CODE_BG);
            self.rect(LEFT, self.y, 1.6, h, RULE);
            let top = self.y + if first { PAD } else { 0.0 };
            self.text(LEFT + PAD + 6.0, top + 8.6, F::Mono, 8.0, INK, &line);
            self.y += h;
        }
    }

    fn block(&mut self, b: &PdfBlock) {
        match b {
            PdfBlock::Heading { text, count, tone } => self.heading(text, count.as_deref(), *tone),
            PdfBlock::Paragraph { text, tone } => {
                self.y += 4.0;
                self.lines(LEFT, F::Body, 9.0, 13.0, color(*tone), text);
                self.y += 4.0;
            }
            PdfBlock::List { items, tone } => {
                for item in items {
                    self.ensure(13.0);
                    self.text(LEFT, self.y + 9.9, F::Body, 9.0, color(*tone), "–");
                    self.lines(LEFT + 11.0, F::Body, 9.0, 13.0, INK, item);
                }
                self.y += 4.0;
            }
            PdfBlock::Finding {
                severity,
                tone,
                category,
                title,
                what,
                subjects_label,
                subjects,
                fix_label,
                fix,
            } => self.finding(
                severity,
                *tone,
                category,
                title,
                what,
                (subjects_label, subjects),
                (fix_label, fix.as_deref()),
            ),
        }
    }

    fn footers(&mut self, footer: &str, pages: &str) {
        let total = self.pages.len();
        for i in 0..total {
            let number = pages
                .replace("{n}", &(i + 1).to_string())
                .replace("{total}", &total.to_string());
            let w = self.width(F::Body, 7.5, &number);
            let foot = footer.to_owned();
            let num = number.clone();
            let page = &mut self.pages[i];
            page.push(Op::Rect {
                x: LEFT,
                y: H - 50.0,
                w: WIDTH,
                h: 0.4,
                rgb: RULE,
            });
            page.push(Op::Text {
                x: LEFT,
                y: H - 38.0,
                face: F::Body,
                size: 7.5,
                rgb: MUTED,
                text: foot,
            });
            page.push(Op::Text {
                x: RIGHT - w,
                y: H - 38.0,
                face: F::Body,
                size: 7.5,
                rgb: MUTED,
                text: num,
            });
        }
    }
}

fn fill(rgb: Rgb) -> Fill {
    Fill {
        paint: rgb::Color::new(rgb.0, rgb.1, rgb.2).into(),
        ..Fill::default()
    }
}

pub fn render(report: &PdfReport) -> Result<Vec<u8>, PdfError> {
    let faces =
        Faces::of(report.style).ok_or_else(|| PdfError::Write("bundled font".to_owned()))?;
    let mut l = Layout {
        faces: &faces,
        style: report.style,
        title: report.title.clone(),
        pages: vec![Vec::new()],
        y: TOP,
    };
    l.head(report);
    for b in &report.blocks {
        l.block(b);
    }
    l.footers(&report.footer, &report.pages);

    let mut doc = Document::new();
    doc.set_metadata(
        Metadata::new()
            .title(report.title.clone())
            .creator("NiceDB".to_owned())
            .language(report.language.clone()),
    );
    let settings = PageSettings::from_wh(W, H).ok_or_else(|| PdfError::Write("page".into()))?;
    for ops in &l.pages {
        let mut page = doc.start_page_with(settings.clone());
        let mut surface = page.surface();
        for op in ops {
            match op {
                Op::Text {
                    x,
                    y,
                    face,
                    size,
                    rgb,
                    text,
                } => {
                    surface.set_fill(Some(fill(*rgb)));
                    let mut at = *x;
                    for (f, piece) in runs(l.face(*face), &l.faces.fallback, text) {
                        surface.draw_text(
                            Point::from_xy(at, *y),
                            f.font.clone(),
                            *size,
                            &piece,
                            false,
                            TextDirection::Auto,
                        );
                        at += f.width(&piece, *size);
                    }
                }
                Op::Rect { x, y, w, h, rgb } => {
                    let Some(rect) = Rect::from_xywh(*x, *y, *w, *h) else {
                        continue;
                    };
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        surface.set_fill(Some(fill(*rgb)));
                        surface.draw_path(&path);
                    }
                }
            }
        }
        surface.finish();
        page.finish();
    }
    doc.finish().map_err(|e| PdfError::Write(e.to_string()))
}
