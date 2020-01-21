package de.sve.backend.model;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class EventBooking {

	public abstract String eventId();

	public abstract String firstName();

	public abstract String lastName();

	public abstract String street();

	public abstract String city();

	public abstract String email();

	public abstract String phone();

	public abstract Boolean member();

	public abstract Boolean updates();

	public abstract String comments();

}
