<template>
  <Layout>
    <v-container>
      <action-header
        title="Batch Dokument"
        subtitle="Individuelles Dokument per Email"
      />
      <v-row>
        <v-col cols="12">
          <v-form :disabled="disabled">
            <from-select ref="from" v-model="from" @preset="preset" />
            <v-file-input
              v-if="people.length <= 0"
              outlined
              multiple
              @change="fileSelection"
              prepend-icon=""
              label="Dokumente auswählen"
            ></v-file-input>
            <people-field v-else :disabled="disabled" v-model="people" />
            <v-text-field
              v-model="filename"
              outlined
              label="Dokumentname in Email"
            ></v-text-field>
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
    title: 'Batch Dokument',
  },
  data() {
    return {
      from: 'Fitness',
      files: [],
      people: [],
      filename: '',
      subject: '',
      content: '',
      disabled: false,
      progress: 0,
    }
  },
  methods: {
    fileSelection(files) {
      const items = []
      for (const file of files) {
        const name = file.name.split('.').slice(0, -1).join('.')
        var parts = name.split('#')
        if (parts.length !== 3) {
          console.log('File could not be splitted into 3 parts: ' + parts)
          this.$refs.notify.showError(
            'Datei ' +
              file.name +
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
          file: file,
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
        const attachment = {
          name: this.filename,
          mimeType: person.file.type,
        }
        try {
          attachment.data = await this.readFile(person.file)
        } catch (error) {
          console.error(error)
          this.$refs.notify.showError(
            'Datei konnte nicht gelesen werden. Details siehe Console'
          )
          this.disabled = false
          return
        }

        const data = {
          type: this.from,
          to: person.email,
          subject: this.subject,
          content: replace(this.content, person),
          attachments: [attachment],
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
    readFile(inputFile) {
      const reader = new FileReader()

      return new Promise((resolve, reject) => {
        reader.onerror = () => {
          reader.abort()
          reject(new DOMException('Error reading file: ' + reader.error))
        }

        reader.onload = () => {
          var dataUrl = reader.result
          var base64 = dataUrl.split(',')[1]
          resolve(base64)
        }
        reader.readAsDataURL(inputFile)
      })
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
