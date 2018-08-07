fetch('http://example.org')
  .then(r => r.text())
  .then(body => {
    // Log first line
    console.log(body.slice(body.indexOf('\n')))
  })
