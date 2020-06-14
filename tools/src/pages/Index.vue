<template>
  <Layout :sidebar="false">
    <v-container class="fill-height" fluid>
      <v-row align="center" justify="center">
        <v-col cols="12" sm="8" md="4">
          <v-text-field
            label="Passwort"
            name="password"
            type="password"
            v-model="password"
            outlined
            @keyup.enter="login"
          ></v-text-field>
        </v-col>
      </v-row>
    </v-container>
  </Layout>
</template>

<script>
import Cookies from 'js-cookie'
import Vue from 'vue'

export default {
  metaInfo: {
    title: 'Login',
  },
  props: {
    encrypt: {
      type: String,
      default: 'd29ya0BzdmU=', // work@sve
    },
  },
  data() {
    return {
      password: '',
    }
  },
  methods: {
    login() {
      if (this.encrypt === window.btoa(this.password)) {
        if (process.isClient) {
          Cookies.set('sve_backend_tools', 'verified', { expires: 28 })
          this.$router.push(
            this.$page.metadata.pathPrefix +
              this.$page.actions.edges[0].node.link
          )
        }
      }
    },
  },
}
</script>

<page-query>
  query {
    metadata {
      pathPrefix
    }
    actions: allActions(order: ASC) {
      edges {
        node {
          link
        }
      }
    }
  }
</page-query>
