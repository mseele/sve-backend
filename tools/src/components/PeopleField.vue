<template>
  <v-card class="pa-1 mb-6 colored-border" outlined>
    <template v-for="(person, index) in people">
      <v-chip
        class="ma-2"
        :class="{ primary: !disabled }"
        :close="!disabled"
        @click:close="close(index)"
        :key="index"
      >
        {{ person.firstName }} {{ person.lastName }}
      </v-chip>
    </template>
  </v-card>
</template>

<style lang="scss">
.colored-border {
  border-color: rgba(0, 0, 0, 0.42) !important;
}
</style>

<script>
export default {
  props: {
    disabled: {
      type: Boolean,
      default: false,
    },
    value: {
      type: Array,
      default: [],
    },
  },
  data() {
    return {
      people: this.value.sort(this.compare),
    }
  },
  methods: {
    close(index) {
      this.people.splice(index, 1)
      this.$emit('input', this.people)
    },
    compare(a, b) {
      const personA = a.firstName.toUpperCase() + ' ' + a.lastName.toUpperCase()
      const personB = b.firstName.toUpperCase() + ' ' + b.lastName.toUpperCase()
      return personA.localeCompare(personB)
    },
  },
}
</script>
