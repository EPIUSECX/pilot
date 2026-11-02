const [major, minor] = process.versions.node.split('.').map(Number)
if (major < 20 || (major === 20 && minor < 19)) {
  console.error(
    `Pilot Desktop needs Node.js 20.19 or later (frappe-ui 1.x). Found ${process.versions.node}.`,
  )
  process.exit(1)
}
