// 简历排版；日常内容统一编辑 cv.typ。
#let blue = rgb("#17648f")
#let blue-dark = rgb("#0f4e72")
#let ink = rgb("#20272c")
#let muted = rgb("#68747c")
#let orange = rgb("#ff6a00")
#let sans = sys.inputs.at("body-font", default: "Noto Sans CJK SC")
#let body-font = sans
#let serif-font = sys.inputs.at("serif-font", default: "Noto Serif CJK SC")
#let venue-column-width = 118pt

#let section(title) = {
  v(8pt)
  grid(
    columns: (auto, 1fr),
    column-gutter: 8pt,
    align: horizon,
    text(title, font: sans, fill: blue, weight: "semibold", size: 15.3pt, tracking: 0.035em),
    line(length: 100%, stroke: 0.7pt + blue),
  )
  v(4pt)
}

#let bullet(body) = {
  grid(
    columns: (6pt, 1fr),
    column-gutter: 5pt,
    align: top,
    text([•], fill: blue, weight: "bold", size: 8.5pt),
    body,
  )
  v(2.7pt)
}

#let education(entry) = {
  grid(
    columns: (1fr, auto),
    column-gutter: 10pt,
    align: (left + horizon, right + horizon),
    text(entry.school, font: sans, fill: blue-dark, weight: "semibold", size: 12.7pt),
    text(entry.date, font: sans, fill: muted, weight: "medium", size: 10.2pt),
  )
  v(1pt)
  text(entry.degree, font: sans, weight: "semibold", size: 11.4pt)
  v(1pt)
  text(entry.detail, size: 10.7pt, fill: ink)
  v(6pt)
}

#let venue-heading(venue, status) = {
  text(venue, font: sans, fill: blue-dark, weight: "bold", size: 13pt, tracking: 0.02em)
  h(3pt)
  text(status, font: serif-font, fill: muted, weight: "regular", size: 9pt, tracking: 0.06em)
}

#let experience(entry) = {
  let internship = entry.kind == "internship"
  grid(
    columns: (3pt, 1fr, venue-column-width),
    column-gutter: 7pt,
    row-gutter: 3pt,
    align: (center + top, left + top, right + top),
    rect(width: 3pt, height: if internship { 16pt } else { 15pt }, fill: if internship { orange } else { blue }, radius: 1.5pt),
    text(
      entry.org,
      font: sans,
      fill: blue-dark,
      weight: "bold",
      size: if internship { 14.4pt } else { 13.4pt },
      tracking: if internship { 0.018em } else { 0.012em },
    ),
    venue-heading(entry.venue, entry.status),
    [],
    text(entry.role, font: sans, fill: muted, weight: "medium", size: 10.4pt),
    text(entry.date, font: sans, fill: muted, weight: "medium", size: 9.9pt),
  )
  block(inset: (left: 10pt))[
    #v(1.5pt)
    #text(entry.paper_title, font: sans, fill: ink, weight: "bold", size: 12.2pt)
  ]
  v(4pt)
  for item in entry.bullets {
    bullet(item)
  }
  v(6pt)
}

#let render-resume(profile, phone: "") = {
  set page(paper: "a4", margin: (x: 1.45cm, y: 1.15cm))
  set text(font: body-font, size: 11.4pt, fill: ink, tracking: 0.008em)
  set par(leading: 0.90em, spacing: 0.5em)
  show link: set text(fill: ink)

  grid(
    columns: (1fr, auto),
    column-gutter: 18pt,
    align: (left + bottom, right + bottom),
    [
      #text(profile.display_name, font: sans, fill: blue-dark, weight: "semibold", size: 28.5pt, tracking: 0.01em)
      #h(8pt)
      #text(profile.english_name, font: sans, fill: muted, weight: "semibold", size: 12.2pt, tracking: 0.01em)
      #linebreak()
      #text(profile.tagline, font: sans, fill: muted, weight: "medium", size: 10.8pt)
    ],
    [
      #align(right)[
        #text(profile.location, font: sans, weight: "semibold", size: 10.1pt)
        #linebreak()
        #text(font: sans, size: 9.7pt)[
          #if phone != "" {
            link("tel:" + phone.replace(" ", ""), phone)
            [ · ]
          }
          #link("mailto:" + profile.email, profile.email)
        ]
      ]
    ],
  )
  v(5pt)
  line(length: 100%, stroke: 1.25pt + blue)

  if profile.education.len() > 0 {
    section([教育背景])
    for entry in profile.education {
      education(entry)
    }
  }

  let internships = profile.experiences.filter(entry => entry.kind == "internship")
  if internships.len() > 0 {
    v(0.06fr)
    section([实习经历])
    for entry in internships {
      experience(entry)
    }
  }

  let research = profile.experiences.filter(entry => entry.kind == "research")
  if research.len() > 0 {
    v(0.06fr)
    section([科研经历])
    for entry in research {
      experience(entry)
    }
  }

  if profile.honors.len() > 0 {
    v(0.06fr)
    section([荣誉])
    grid(
      columns: (1.25fr, 1fr, 0.7fr),
      column-gutter: 14pt,
      row-gutter: 5pt,
      align: left + horizon,
      ..profile.honors.map(entry => [
        #text(entry.year, font: sans, fill: blue-dark, weight: "semibold", size: 10.4pt)
        #h(5pt)
        #text(entry.title, font: sans, size: 10.4pt)
      ]),
    )
  }
}
