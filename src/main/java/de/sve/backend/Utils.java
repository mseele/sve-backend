package de.sve.backend;

import java.io.IOException;
import java.time.LocalDateTime;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.TypeAdapter;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
import com.google.gson.stream.JsonWriter;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

public class Utils {

	public static Gson gson() {
		return new GsonBuilder().registerTypeAdapterFactory(GenerateTypeAdapter.FACTORY)
								.registerTypeAdapter(LocalDateTime.class, new LocalDateTimeAdapter())
								.create();
	}

	private static class LocalDateTimeAdapter extends TypeAdapter<LocalDateTime> {

		@Override
		public void write(final JsonWriter jsonWriter, final LocalDateTime date) throws IOException {
			if (date == null) {
				jsonWriter.nullValue();
			} else {
				jsonWriter.value(date.toString());
			}
		}

		@Override
		public LocalDateTime read(final JsonReader jsonReader) throws IOException {
			if (jsonReader.peek() == JsonToken.NULL) {
				jsonReader.nextNull();
				return null;
			}
			return LocalDateTime.parse(jsonReader.nextString());
		}

	}

}
