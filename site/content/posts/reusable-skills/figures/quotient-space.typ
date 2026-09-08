#import "@preview/cetz:0.4.2"

#set page(width: auto, height: auto, margin: 1em)

#cetz.canvas({
  import cetz.draw: *

  // 原始轨迹空间
  rect((0, 0), (4, 3), stroke: 1pt + gray, name: "traj-space")
  content((2, 3.5), text(12pt)[原始轨迹空间 $cal(T)$])

  // 轨迹点
  circle((0.5, 0.5), radius: 0.08, fill: blue)
  circle((1.2, 0.7), radius: 0.08, fill: blue)
  circle((0.8, 1.5), radius: 0.08, fill: blue)
  circle((2.5, 0.8), radius: 0.08, fill: red)
  circle((3.0, 1.2), radius: 0.08, fill: red)
  circle((1.5, 2.2), radius: 0.08, fill: green)
  circle((2.2, 2.5), radius: 0.08, fill: green)
  circle((3.5, 2.0), radius: 0.08, fill: green)

  // 投影箭头
  line((4.5, 1.5), (6, 1.5), mark: (end: ">"), stroke: 2pt)
  content((5.25, 2), text(11pt)[$pi$])

  // 商空间
  rect((6.5, 0), (10.5, 3), stroke: 1pt + gray, name: "quotient-space")
  content((8.5, 3.5), text(12pt)[技能商空间 $cal(T) slash tilde$])

  // 等价类
  circle((7.2, 1), radius: 0.3, stroke: 2pt + blue, fill: blue.lighten(80%))
  content((7.2, 1), text(10pt)[$[tau_1]$])

  circle((9.5, 1.5), radius: 0.3, stroke: 2pt + red, fill: red.lighten(80%))
  content((9.5, 1.5), text(10pt)[$[tau_2]$])

  circle((8.2, 2.2), radius: 0.3, stroke: 2pt + green, fill: green.lighten(80%))
  content((8.2, 2.2), text(10pt)[$[tau_3]$])
})
