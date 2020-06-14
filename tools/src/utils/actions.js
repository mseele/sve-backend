function validateEmail(email) {
  const re = /^(([^<>()\[\]\.,;:\s@\"]+(\.[^<>()\[\]\.,;:\s@\"]+)*)|(\".+\"))@(([^<>()[\]\.,;:\s@\"]+\.)+[^<>()[\]\.,;:\s@\"]{2,})$/i
  return re.test(String(email).toLowerCase())
}

function replace(content, person) {
  return content
    .replace('${vorname}', person.firstName)
    .replace('${firstName}', person.firstName)
    .replace('${nachname}', person.lastName)
    .replace('${lastName}', person.lastName)
}

export { validateEmail, replace }
