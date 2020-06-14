const VuetifyLoaderPlugin = require('vuetify-loader/lib/plugin')

module.exports = function (api) {
  api.loadSource((store) => {
    store.addMetadata(
      'sendEmailURL',
      'https://backend.sv-eutingen.de/api/contact/email'
    )
  })

  api.loadSource(({ addCollection }) => {
    const actions = require('./src/data/actions.json')
    const collection = addCollection('actions')
    for (const action of actions) {
      collection.addNode(action)
    }
  })

  api.chainWebpack((config, { isServer }) => {
    config.plugin('vuetify-loader').use(VuetifyLoaderPlugin)
  })
}
