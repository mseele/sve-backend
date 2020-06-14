import Vuetify from 'vuetify/lib'
import 'vuetify/dist/vuetify.min.css'
import Cookies from 'js-cookie'
import DefaultLayout from '~/layouts/Default.vue'

export default function (Vue, { appOptions, head, router }) {
  head.link.push({
    rel: 'stylesheet',
    href:
      'https://fonts.googleapis.com/css?family=Roboto:100,300,400,500,700,900',
  })

  Vue.use(Vuetify)

  appOptions.vuetify = new Vuetify({
    icons: {
      iconfont: 'mdiSvg',
    },
  })

  if (process.isClient) {
    router.beforeEach((to, from, next) => {
      if (Cookies.get('sve_backend_tools') === 'verified' || to.path === '/') {
        next()
      } else {
        next('/')
      }
    })
  }

  Vue.component('Layout', DefaultLayout)
}
