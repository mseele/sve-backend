<template>
  <Layout>
    <v-container>
      <action-header title="Batch Email" subtitle="Individuelle Email" />
      <v-row>
        <v-col cols="12">
          <v-form :disabled="disabled">
            <from-select ref="from" v-model="from" @preset="preset" />
            <v-text-field
              v-if="people.length <= 0"
              outlined
              label="Vorname, Nachname u. Email Spalten aus Excel kopieren"
              @paste="paste"
            ></v-text-field>
            <people-field v-else :disabled="disabled" v-model="people" />
            <v-text-field
              v-model="subject"
              outlined
              label="Betreff"
            ></v-text-field>
            <v-textarea v-model="content" outlined label="Email"></v-textarea>
          </v-form>
          <button-area
            :disabled="disabled"
            :people="people"
            :progress="progress"
            @send="send"
            @reset="reset"
          />
        </v-col>
      </v-row>
    </v-container>
    <notify ref="notify" />
  </Layout>
</template>

<style lang="scss">
.colored-border {
  border-color: rgba(0, 0, 0, 0.42) !important;
}
</style>

<script>
import ActionHeader from '~/components/ActionHeader.vue'
import Notify from '~/components/Notify.vue'
import PeopleField from '~/components/PeopleField.vue'
import ButtonArea from '~/components/ButtonArea.vue'
import FromSelect from '~/components/FromSelect.vue'
import { validateEmail, replace } from '~/utils/actions.js'
import axios from 'axios'

export default {
  components: {
    ActionHeader,
    Notify,
    PeopleField,
    ButtonArea,
    FromSelect,
  },
  metaInfo: {
    title: 'Batch Email',
  },
  data() {
    return {
      from: 'Fitness',
      people: [],
      subject: '',
      content: '',
      disabled: false,
      progress: 0,
    }
  },
  methods: {
    paste(event) {
      var clipboardData =
        event.clipboardData ||
        event.originalEvent.clipboardData ||
        window.clipboardData
      const text = clipboardData.getData('text')
      const items = []
      const lines = text.match(/[^\r\n]+/g)
      for (const line of lines) {
        var parts = line.split('\t')
        if (parts.length !== 3) {
          console.log('Line could not be splitted into 3 parts: ' + parts)
          this.$refs.notify.showError(
            'Zeile ' +
              line +
              ' konnte nicht gelesen werden. Details siehe Console'
          )
          return
        }
        if (!validateEmail(parts[2])) {
          console.log('Email address is not valid: ' + parts[2])
          this.$refs.notify.showError(
            'Email Adresse ' +
              parts[2] +
              ' ist inkorrekt. Details siehe Console'
          )
          return
        }
        items.push({
          firstName: parts[0],
          lastName: parts[1],
          email: parts[2],
        })
      }
      if (items.length == 0) {
        console.log('No items found in text: ' + text)
        this.$refs.notify.showError(
          'Der kopierte Text konnte nicht verarbeitet werden. Details siehe Console'
        )
        return
      }
      this.people = items
    },
    preset(preset) {
      this.subject = preset.subject
      this.content = preset.content
    },
    reset() {
      this.$refs.from.reset()
      this.people = []
      this.disabled = false
    },
    async send() {
      this.disabled = true
      const tick = 100 / this.people.length
      for (const person of this.people) {
        const data = {
          type: this.from,
          to: person.email,
          subject: this.subject,
          content: replace(this.content, person),
        }
        try {
          await axios.post(this.$page.metadata.sendEmailURL, data)
        } catch (error) {
          console.error(error)
          this.$refs.notify.showError(
            'Senden fehlgeschlafen. Details siehe Console'
          )
          this.disabled = false
          return
        }
        this.progress += tick
      }
      this.$refs.notify.showSuccess('Alle Emails wurden erfolgreich versandt')
      this.reset()
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
