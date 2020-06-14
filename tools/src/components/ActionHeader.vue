<template>
  <v-row>
    <v-col cols="12">
      <div class="text-h5 text-center">
        {{ title }}
      </div>
      <div class="text-overline text-center">
        {{ subtitle }}
      </div>
    </v-col>
  </v-row>
</template>

<script>
export default {
  props: {
    title: {
      type: String,
      default: '',
    },
    subtitle: {
      type: String,
      default: '',
    },
  },
}
</script>
