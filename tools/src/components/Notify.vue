<template>
  <v-snackbar v-model="snackbar" bottom :color="snackbarColor">
    {{ snackbarText }}
    <template v-slot:action="{ attrs }">
      <v-btn dark text v-bind="attrs" @click="snackbar = false">
        Close
      </v-btn>
    </template>
  </v-snackbar>
</template>

<script>
export default {
  data() {
    return {
      snackbar: false,
      snackbarColor: '',
      snackbarText: '',
    }
  },
  methods: {
    showSuccess(text) {
      this.snackbarText = text
      this.snackbarColor = 'success'
      this.snackbar = true
    },
    showError(text) {
      this.snackbarText = text
      this.snackbarColor = 'error'
      this.snackbar = true
    },
  },
}
</script>
