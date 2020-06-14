<template>
  <div class="d-flex align-center">
    <div v-if="!disabled && people.length > 0">
      {{
        people.length +
        (people.length == 1
          ? ' Email wird versandt'
          : ' Emails werden verstandt')
      }}
    </div>
    <v-progress-circular
      v-if="disabled"
      size="32"
      rotate="-90"
      :value="progress"
      color="light-blue"
    >
      {{ progress }}
    </v-progress-circular>
    <v-spacer />
    <v-btn depressed class="mr-2" :disabled="disabled" @click="reset()">
      Zurücksetzen
    </v-btn>
    <v-btn
      depressed
      :color="confirmSend ? 'red' : 'primary'"
      :disabled="disabled"
      @click="send()"
    >
      {{ confirmSend ? 'Sicher?' : 'Senden' }}
    </v-btn>
  </div>
</template>

<script>
export default {
  props: {
    progress: {
      type: Number,
      default: 0,
    },
    disabled: {
      type: Boolean,
      default: false,
    },
    people: {
      type: Array,
      default: [],
    },
  },
  data() {
    return {
      confirmSend: false,
    }
  },
  methods: {
    reset() {
      this.confirmSend = false
      this.$emit('reset')
    },
    async send() {
      if (!this.confirmSend) {
        this.confirmSend = true
        return
      }
      this.confirmSend = false
      this.$emit('send')
    },
  },
}
</script>
