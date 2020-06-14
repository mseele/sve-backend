<template>
  <v-radio-group v-model="from">
    <v-radio
      v-for="from in fromItems"
      :key="from"
      :label="from"
      :value="from"
      :disabled="disabled"
    ></v-radio>
  </v-radio-group>
</template>

<script>
export default {
  props: {
    value: {
      type: String,
      default: 'Fitness',
    },
    disabled: {
      type: Boolean,
      default: false,
    },
  },
  data() {
    return {
      from: this.value,
      fromItems: ['Fitness', 'Events'],
    }
  },
  mounted() {
    this.onChange(this.value)
  },
  watch: {
    from(newValue) {
      this.onChange(newValue)
    },
  },
  methods: {
    onChange(newValue) {
      if (newValue === 'Fitness') {
        this.selectFitness()
      } else {
        this.selectEvents()
      }
    },
    selectFitness() {
      this.select({
        subject: '[Fitness@SVE] ',
        content: `

Herzliche Grüße
Team Fitness@SVE`,
      })
    },
    selectEvents() {
      this.select({
        subject: '[Events@SVE] ',
        content: `

Herzliche Grüße
Team Events@SVE`,
      })
    },
    select(preset) {
      this.$emit('input', this.from)
      this.$emit('preset', preset)
    },
    reset() {
      this.from = 'Fitness'
      this.selectFitness()
    },
  },
}
</script>

<page-query>
  query {
    metadata {
      sendEmailURL
    }
  }
</page-query>
