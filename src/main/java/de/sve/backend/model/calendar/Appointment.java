package de.sve.backend.model.calendar;

import java.time.LocalDate;
import java.time.LocalDateTime;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Appointment {

	public static Appointment create(String id, int sortIndex, String title, String link, String description, LocalDate startDate, LocalDate endDate, LocalDateTime startDateTime, LocalDateTime endDateTime) {
		return new AutoValue_Appointment(id, sortIndex, title, link, description, startDate, endDate, startDateTime, endDateTime);
	}

	@Nullable
	public abstract String id();

	public abstract int sortIndex();

	public abstract String title();

	@Nullable
	public abstract String link();

	@Nullable
	public abstract String description();

	@Nullable
	public abstract LocalDate startDate();

	@Nullable
	public abstract LocalDate endDate();

	@Nullable
	public abstract LocalDateTime startDateTime();

	@Nullable
	public abstract LocalDateTime endDateTime();

}
