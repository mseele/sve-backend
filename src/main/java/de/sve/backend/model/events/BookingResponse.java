package de.sve.backend.model.events;

import java.util.Collections;
import java.util.List;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class BookingResponse {

	public static BookingResponse success(String message, List<EventCounter> counter) {
		return new AutoValue_BookingResponse(Boolean.TRUE, message, counter);
	}

	public static BookingResponse failure(String message) {
		return new AutoValue_BookingResponse(Boolean.FALSE, message, Collections.emptyList());
	}

	public abstract Boolean success();

	public abstract String message();

	public abstract List<EventCounter> counter();

}
